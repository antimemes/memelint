"""tools/memelint_suite.py correct|serve|bench VER [--baseline BENCHID] — the sparkDash Quality suite with each prompt
first corrected to memelint (ASD-STE100 Simplified Technical English) by the served model itself:
  correct  dump the suite (tools/quality/dump_suite.mjs, sparkDash's own generator), lint each prompt with memelint
           (--profile, default basic, memelint's own default), and have the model on --upstream rewrite each prompt
           that has findings, lint the rewrite, and repeat (at most --rounds model calls, c--workers). A rewrite must
           keep the numbers, quoted strings, follow-rule phrases, MMLU option labels and the long-context name list,
           or it is rejected; these strings are linted as code. The best rewrite (fewest findings, fewer than the
           original) is kept, else the original. A long item's haystack is data: only its instruction is corrected.
           -> results/VER/suite.json, corrections.json, memelint.toml
  serve    proxy 127.0.0.1:--port -> --upstream that puts each suite prompt's correction in its place; other requests
           go through as they are
  bench    serve in-process, add --port to spark1's llmPorts, run sparkDash's Quality bench on it with label VER and
           the r3.1 run's settings (all 8 categories, long 8k-256k x2, c--concurrency), save it, compare it paired
           with --baseline, then take --port out of llmPorts again. sparkDash sends, times and scores every item.
           -> results/VER/quality-1.json, baseline-quality.json, compare.txt, one line in results/index.jsonl
Each rewritten request -> results/VER/proxy.jsonl."""
import argparse, collections, hashlib, http.client, http.server, json, math, os, re, subprocess, tempfile, threading, time
import urllib.parse, urllib.request
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DASH = "http://127.0.0.1:5555/api"
CATS = ["qa", "reason", "arith", "track", "gsm8k", "mmlu", "follow", "long"]
LONG_SIZES = [8192, 16384, 32768, 65536, 131072, 262144]
LONG_SPLIT = "\n\nSome codes were corrected later in the text;"
FINDING = re.compile(r"^(.+?):(\d+):(\d+): (error|warning): (.*) \[STE ([\d.]+)\]$")
SUMMARY = re.compile(r"^\d+ files?, \d+ comments?, ")
QUOTED = re.compile(r"""(?<![\w'"])(['"])[^'"\n]{1,120}?\1(?!\w)""")
NUMBER = re.compile(r"\d+(?:[.,]\d+)*")
ENUM = re.compile(r"\(\d+\)|^\d+\.(?=\s)", re.M)  # step labels "(3)", rule numbers "3. ": a rewrite may drop them
OPTION = re.compile(r"^[A-D]\. ", re.M)
RULE = re.compile(r"^\d+\. ", re.M)
HOP = {"connection", "keep-alive", "proxy-connection", "transfer-encoding", "te", "trailer", "upgrade"}
SYSTEM = ("You are a technical editor. Rewrite the user's text so that it obeys ASD-STE100 Simplified Technical English, "
          "as checked by the linter memelint. Fix every memelint finding that is listed. Rules: keep the meaning, the task, "
          "every number, name and fact exactly the same; change only the words and sentences that the findings name, plus "
          "what is necessary to fix them; split a long sentence into shorter sentences (25 words or fewer each); replace an "
          "-ing word with an approved form (for example 'containing' -> 'that contains'); write contractions in full. "
          "Copy each protected string character for character. Do not answer, solve or comment on the text. "
          "Output only the rewritten text.")

ap = argparse.ArgumentParser()
ap.add_argument("cmd", choices=["correct", "serve", "bench"]); ap.add_argument("ver")
ap.add_argument("--memelint", default=os.environ.get("MEMELINT", "memelint")); ap.add_argument("--profile", default="basic")
ap.add_argument("--rounds", type=int, default=4); ap.add_argument("--workers", type=int, default=8)
ap.add_argument("--upstream", default="http://127.0.0.1:8888"); ap.add_argument("--port", type=int, default=8899)
ap.add_argument("--concurrency", type=int, default=8); ap.add_argument("--baseline")
a = ap.parse_args()
out = os.path.join(ROOT, "results", a.ver)
os.makedirs(out, exist_ok=True)
CONFIG = os.path.join(out, "memelint.toml")


def log(*x):
    print(time.strftime("%H:%M:%S"), *x, flush=True)


def sha(s):
    return hashlib.sha256(s.encode()).hexdigest()


def http_json(method, url, body=None, timeout=60):
    data = json.dumps(body).encode() if body is not None else None
    r = urllib.request.Request(url, data=data, method=method, headers={"content-type": "application/json"})
    with urllib.request.urlopen(r, timeout=timeout) as f:
        return json.load(f)


# ─── memelint ───

def lint(text):
    """memelint findings of one prompt, linted as a Markdown file."""
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "prompt.md")
        with open(path, "w") as f:
            f.write(text + "\n")
        r = subprocess.run([a.memelint, "check", "--config", CONFIG, path], capture_output=True, text=True)
    lines = (r.stdout + r.stderr).splitlines()
    if r.returncode not in (0, 1) or not any(SUMMARY.match(x) for x in lines):
        raise RuntimeError(f"memelint failed ({r.returncode}): {r.stderr.strip()[:300]}")
    src = text.split("\n")
    found = []
    for x in lines:
        m = FINDING.match(x)
        if m:
            ln, col = int(m[2]), int(m[3])
            at = src[ln - 1][col - 1:col + 59].replace("`", "") if ln <= len(src) else ""
            found.append({"rule": m[6], "severity": m[4], "message": m[5], "line": ln, "col": col, "at": at})
    return found


def protect(item):
    """Strings a rewrite must keep verbatim, and the long-context name list."""
    p = item["prompt"]
    lits = {m.group(0) for m in QUOTED.finditer(p)}
    lits |= {r["e"] for r in item.get("rules") or [] if r.get("kind") == "ends-with"}
    if "***" in p:
        lits.add("***")
    block = p.split("\n", 1)[1] if item["category"] == "long" else None
    return sorted(lits, key=len, reverse=True), block


def masked(text, lits, block):
    """The text that memelint sees: protected strings as code, which memelint does not lint."""
    if block and text.endswith("\n" + block):
        text = text[:-len(block)] + "```\n" + block + "\n```"
    if lits:
        text = re.sub("|".join(map(re.escape, lits)), lambda m: f"`{m.group(0)}`", text)
    return text


# ─── correction by the model ───

def chat(messages, max_tokens):
    body = {"model": a.model, "messages": messages, "max_tokens": max_tokens, "temperature": 0, "top_p": 1, "seed": 1234,
            "stream": False, "chat_template_kwargs": {"enable_thinking": False, "thinking": False, "thinking_mode": "disabled"}}
    for n in range(3):
        try:
            j = http_json("POST", a.upstream + "/v1/chat/completions", body, timeout=900)
            c = j["choices"][0]
            return (c["message"].get("content") or "").strip(), c.get("finish_reason"), j.get("usage") or {}
        except (OSError, KeyError, ValueError) as e:
            if n == 2:
                raise
            log("retry after", repr(e)[:200])
            time.sleep(10)


def unwrap(t):
    t = re.sub(r"^```[\w-]*\n(.*)\n```$", r"\1", t.strip(), flags=re.S)
    t = re.sub(r"^(?:TEXT:\s*)?<<<\n?", "", t)
    return re.sub(r"\n?>>>$", "", t).strip()


def ask(cur, findings, lits, block, notes):
    u = f"TEXT:\n<<<\n{cur}\n>>>\n\nMEMELINT FINDINGS:\n" + "\n".join(
        f"- {f['message']} [STE {f['rule']}] (at: \"{f['at']}\")" for f in findings)
    if lits:
        u += "\n\nPROTECTED STRINGS (copy each one exactly, with its quotation marks):\n" + "\n".join(f"- {x}" for x in lits)
    if block:
        u += "\n\nThe list of names at the end of the text is data: keep it exactly as it is, one name on each line."
    for n in notes:
        u += f"\n\nA rewrite was rejected: {n}. Rewrite the text again."
    return [{"role": "system", "content": SYSTEM}, {"role": "user", "content": u}]


def guard(item, cand, lits, block):
    """Why a rewrite changes the task, or None."""
    o = item["prompt"]
    if not cand:
        return "the rewrite was empty"
    gone = [x for x in lits if cand.count(x) < o.count(x)]
    if gone:
        return "these protected strings are missing or changed: " + " ; ".join(gone)
    if block and not cand.endswith("\n" + block):
        return "the list of names at the end is changed; keep it exactly as it is, one name on each line, at the end"
    lost = collections.Counter(NUMBER.findall(ENUM.sub(" ", o))) - collections.Counter(NUMBER.findall(cand))
    if lost:
        return "these numbers are missing or changed: " + ", ".join(sorted(lost))
    if cand.count("Answer:") > o.count("Answer:"):
        return "the rewrite answers the question; do not answer it, only rewrite the text"
    if OPTION.findall(cand) != OPTION.findall(o):
        return "keep the options as lines that start with 'A. ', 'B. ', 'C. ' and 'D. '"
    if len(RULE.findall(cand)) != len(RULE.findall(o)):
        return "keep each numbered rule on its own numbered line"
    ratio = len(cand) / len(o)
    if not 0.5 <= ratio <= 2.5:
        return f"the rewrite is {ratio:.1f} times the length of the text; keep all of the content and add nothing"
    return None


def correct_item(item):
    lits, block = protect(item)
    o = item["prompt"]
    f0 = lint(masked(o, lits, block))
    rec = {"id": item["id"], "category": item["category"], "protected": lits, "original": o, "before": f0, "attempts": []}
    best, cur, curf, notes = None, o, f0, []
    for _ in range(a.rounds if f0 else 0):
        text, finish, usage = chat(ask(cur, curf, lits, block, notes), 256 + len(cur) // 2)
        cand = unwrap(text)
        why = guard(item, cand, lits, block)
        att = {"text": cand, "finish": finish, "tokens": usage.get("completion_tokens"), "rejected": why}
        rec["attempts"].append(att)
        if why:
            notes.append(why)
            continue
        att["findings"] = fc = lint(masked(cand, lits, block))
        if best is None or len(fc) < len(best[1]):
            best = (cand, fc)
        if not fc:
            break
        cur, curf, notes = cand, fc, []
    if not f0:
        status, final = "clean", (o, [])
    elif best and len(best[1]) < len(f0):
        status, final = ("corrected" if not best[1] else "partial"), best
    else:
        status, final = "kept", (o, f0)
    rec.update(status=status, corrected=final[0], after=final[1])
    return rec


def cmd_correct():
    with open(f"{out}/suite.json", "w") as f, open(f"{ROOT}/tools/quality/dump_suite.mjs") as src:
        subprocess.run(["docker", "exec", "-i", "sparkDash", "node", "--input-type=module", "-"], stdin=src, stdout=f, check=True)
    suite = json.load(open(f"{out}/suite.json"))
    with open(CONFIG, "w") as f:
        f.write(f'# memelint for the Quality prompts (tools/memelint_suite.py)\nprofile = "{a.profile}"\n\n'
                "[languages.markdown]\ncheck = true\n")
    a.model = http_json("GET", a.upstream + "/v1/models")["data"][0]["id"]
    version = subprocess.run([a.memelint, "--version"], capture_output=True, text=True).stdout.strip()
    log(len(suite), "items;", version, "profile", a.profile, "; model", a.model, "; workers", a.workers)
    t0, done, lock = time.time(), [0], threading.Lock()

    def one(it):
        r = correct_item(it)
        with lock:
            done[0] += 1
            if done[0] % 50 == 0 or done[0] == len(suite):
                log(f"{done[0]}/{len(suite)}")
        return r

    with ThreadPoolExecutor(a.workers) as ex:
        recs = list(ex.map(one, suite))
    meta = {"memelint": version, "profile": a.profile, "model": a.model, "upstream": a.upstream, "rounds": a.rounds,
            "workers": a.workers, "system": SYSTEM, "secs": round(time.time() - t0), "ts": time.strftime("%Y-%m-%dT%H:%M:%S")}
    json.dump({"meta": meta, "items": recs}, open(f"{out}/corrections.json", "w"), indent=1)
    print(summary(recs))


def summary(recs):
    lines = [f"{'cat':7} {'items':>5} {'linted':>6} {'fixed':>5} {'partial':>7} {'kept':>4}  findings before -> after  rejected rewrites"]
    for c in CATS + ["total"]:
        rs = [r for r in recs if c in ("total", r["category"])]
        st = collections.Counter(r["status"] for r in rs)
        rej = sum(bool(x["rejected"]) for r in rs for x in r["attempts"])
        lines.append(f"{c:7} {len(rs):>5} {len(rs) - st['clean']:>6} {st['corrected']:>5} {st['partial']:>7} {st['kept']:>4}  "
                     f"{sum(len(r['before']) for r in rs):>8} -> {sum(len(r['after']) for r in rs):<5}  "
                     f"{rej}/{sum(len(r['attempts']) for r in rs)}")
    return "\n".join(lines)


# ─── proxy ───

class Proxy(http.server.BaseHTTPRequestHandler):
    table, up, logf, lock = {}, ("127.0.0.1", 8888), None, threading.Lock()

    def log_message(self, *_):
        pass

    def do_GET(self):
        self.forward(None)

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length") or 0))
        if self.path.split("?")[0].rstrip("/").endswith("/chat/completions"):
            body = self.rewrite(body)
        self.forward(body)

    def lookup(self, c):
        rec = self.table.get(sha(c))
        if rec:
            return rec["corrected"], rec
        at = c.rfind(LONG_SPLIT)
        rec = self.table.get(sha(c[at + 2:])) if at >= 0 else None
        return (c[:at + 2] + rec["corrected"], rec) if rec else (c, None)

    def rewrite(self, body):
        try:
            j = json.loads(body)
        except ValueError:
            return body
        for m in j.get("messages") or []:
            c = m.get("content")
            if m.get("role") != "user" or not isinstance(c, str):
                continue
            new, rec = self.lookup(c)
            e = {"ts": time.strftime("%Y-%m-%dT%H:%M:%S"), "chars": len(c)}
            if rec:
                m["content"] = new
                e.update(id=rec["id"], status=rec["status"], changed=new != c)
            else:
                e.update(miss=sha(c)[:16], head=c[:80])
            with self.lock:
                self.logf.write(json.dumps(e) + "\n")
                self.logf.flush()
        return json.dumps(j).encode()

    def forward(self, body):
        conn = http.client.HTTPConnection(*self.up, timeout=7200)
        started = False
        try:
            hdrs = {k: v for k, v in self.headers.items() if k.lower() not in HOP | {"host", "content-length"}}
            if body is not None:
                hdrs["Content-Length"] = str(len(body))
            conn.request(self.command, self.path, body=body, headers=hdrs)
            resp = conn.getresponse()
            self.send_response(resp.status, resp.reason)
            started = True
            for k, v in resp.getheaders():
                if k.lower() not in HOP:
                    self.send_header(k, v)
            self.send_header("Connection", "close")
            self.end_headers()
            while chunk := resp.read1(65536):
                self.wfile.write(chunk)
        except (BrokenPipeError, ConnectionResetError):
            pass  # the client went away (cancel / timeout): closing the upstream stops the generation
        except Exception as e:
            if not started:
                self.send_error(502, repr(e)[:200])
        finally:
            conn.close()
            self.close_connection = True


def start_proxy():
    recs = json.load(open(f"{out}/corrections.json"))["items"]
    Proxy.table = {sha(r["original"]): r for r in recs}
    u = urllib.parse.urlsplit(a.upstream)
    Proxy.up = (u.hostname, u.port or 80)
    Proxy.logf = open(f"{out}/proxy.jsonl", "a")
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", a.port), Proxy)
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    changed = sum(r["corrected"] != r["original"] for r in recs)
    log(f"proxy 127.0.0.1:{a.port} -> {a.upstream}: {len(recs)} prompts, {changed} corrected")
    return srv


# ─── sparkDash run + paired comparison ───

def cmd_bench():
    srv = start_proxy()
    spark = [s for s in http_json("GET", DASH + "/sparks")["sparks"] if s["id"] == "spark1"][0]
    ports = list(spark["llmPorts"])
    B = DASH + "/sparks/spark1"
    try:
        http_json("PATCH", B, {"llmPorts": ports + [a.port]})
        log("spark1 llmPorts", ports, "->", ports + [a.port])
        body = {"port": a.port, "categories": CATS, "longSizes": LONG_SIZES, "longItems": 2, "concurrency": a.concurrency,
                "label": a.ver}
        bid = http_json("POST", B + "/llm/quality-bench", body)["benchId"]
        log("started quality-bench", bid, json.dumps(body))
        while True:
            time.sleep(30)
            r = http_json("GET", f"{B}/llm/quality-bench/{bid}")
            p = r.get("progress") or {}
            log(r["status"], f"{p.get('done')}/{p.get('total')}", p.get("message"), (r.get("results") or {}).get("overallPct"))
            if r["status"] in ("completed", "failed", "cancelled"):
                break
        full = http_json("GET", f"{B}/llm/quality-bench/{bid}")
        json.dump(full, open(f"{out}/quality-1.json", "w"), indent=1)
        log("finished", full["status"], full.get("error") or "", "->", f"{out}/quality-1.json")
    finally:
        http_json("PATCH", B, {"llmPorts": ports})
        log("spark1 llmPorts back to", ports)
        srv.shutdown()
    if a.baseline:
        compare(full)


def mcnemar(b, c):
    n, k = b + c, min(b, c)
    return 1.0 if n == 0 else min(1.0, 2 * sum(math.comb(n, i) for i in range(k + 1)) / 2 ** n)


def compare(full):
    base = http_json("GET", f"{DASH}/sparks/spark1/llm/quality-bench/{a.baseline}")
    json.dump(base, open(f"{out}/baseline-quality.json", "w"), indent=1)
    bl = base["config"].get("label") or a.baseline[:8]
    recs = {r["id"]: r for r in json.load(open(f"{out}/corrections.json"))["items"]}
    rows, ref = full["results"]["items"], {x["id"]: x for x in base["results"]["items"]}
    misses = sum("miss" in json.loads(x) for x in open(f"{out}/proxy.jsonl"))
    L = [f"{a.ver} ({full['benchId'][:8]}, {full['results']['overallPct']}%) vs {bl} ({a.baseline[:8]}, "
         f"{base['results']['overallPct']}%), paired by item id; proxy cache misses: {misses}",
         f"{'cat':7} {'this':>9} {bl:>9}   {'corrected prompts':>24}   {'unchanged prompts':>24}"]
    groups = {"all": [], "changed": [], "same": []}
    for c in CATS + ["total"]:
        rs = [r for r in rows if c in ("total", r["category"]) and r["id"] in ref]
        ch = [r for r in rs if recs[r["id"]]["corrected"] != recs[r["id"]]["original"]]
        un = [r for r in rs if recs[r["id"]]["corrected"] == recs[r["id"]]["original"]]
        cell = lambda g: f"{len(g):>3}: {sum(bool(r['ok']) for r in g):>3} vs {sum(bool(ref[r['id']]['ok']) for r in g):>3}"
        L.append(f"{c:7} {sum(bool(r['ok']) for r in rs):>4}/{len(rs):<4} {sum(bool(ref[r['id']]['ok']) for r in rs):>4}/{len(rs):<4}"
                 f"   {cell(ch):>24}   {cell(un):>24}")
        if c == "total":
            groups = {"all": rs, "corrected prompts": ch, "unchanged prompts": un}
    for g, rs in groups.items():
        gained = [r["id"] for r in rs if r["ok"] and not ref[r["id"]]["ok"]]
        lost = [r["id"] for r in rs if not r["ok"] and ref[r["id"]]["ok"]]
        L.append(f"{g}: +{len(gained)} -{len(lost)} (McNemar exact p={mcnemar(len(gained), len(lost)):.3f})  "
                 f"gained {gained}  lost {lost}")
    errs = [f"{r['id']}({r['error']})" for r in rows if r.get("error")]
    L.append("errors: " + (", ".join(errs) if errs else "none"))
    text = "\n".join(L)
    open(f"{out}/compare.txt", "w").write(text + "\n")
    print(text)
    res = full["results"]
    with open(os.path.join(ROOT, "results/index.jsonl"), "a") as f:
        f.write(json.dumps({"ts": time.strftime("%Y-%m-%dT%H:%M:%S"), "ver": a.ver, "kind": "quality-memelint",
                            "name": f"sparkdash-{len(rows)}-memelint-{json.load(open(f'{out}/corrections.json'))['meta']['profile']}",
                            "benchId": full["benchId"], "pass": sum(bool(i["ok"]) for i in rows), "total": len(rows),
                            "overallPct": res.get("overallPct"), "cats": {c: [v["passed"], v["total"]] for c, v in res.get("categories", {}).items()},
                            "baseline": bl, "baselinePct": base["results"]["overallPct"],
                            "correctedPrompts": len(groups["corrected prompts"]), "proxyMisses": misses,
                            "fails": [i["id"] for i in rows if not i["ok"]], "file": f"results/{a.ver}/quality-1.json"}) + "\n")


if a.cmd == "correct":
    cmd_correct()
elif a.cmd == "serve":
    start_proxy()
    threading.Event().wait()
else:
    cmd_bench()
