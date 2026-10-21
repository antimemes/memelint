# The sparkDash Quality bench with prompts that obey memelint

This directory is the record of one test. The question was this: when the prompts of a bench obey memelint, does the number of correct answers increase or decrease?

The bench is the Quality bench of sparkDash, with 807 items. Before the model answered a prompt, the same model made a rewrite of the prompt, until memelint found no problems in it. Then sparkDash did the bench run as usual. At the end, we compared each item with a baseline run of the initial prompts.

## Result

| | Baseline (`r3.1`) | This run (`memelint`) |
|---|---|---|
| Prompts | as sparkDash makes them | 387 of 807 changed by the model |
| memelint findings in the prompts (profile `basic`) | 633 | 20 |
| Correct items | 760 of 807 | 757 of 807 |
| sparkDash score (`overallPct`) | 97.2 % | 96.8 % |
| Items with a different result | | 4 correct only in this run, 7 correct only in the baseline |

- The exact McNemar test on the 11 items with a different result gives p = 0.549. Thus the data does not show a difference.
- All 11 items are in the 387 rewritten prompts. For each of the 420 prompts with no change, the answer text was the same in the two runs, byte for byte. Thus the server gave the same output for the same prompt, and the proxy had no effect on the answers.
- 5 of the 11 items are at the token limit of the item. In 3 of the 11 items, the rewrite changed the meaning of the question or of an option. Refer to [Items with a different result](#items-with-a-different-result).
- The two 256k items had an error in the two runs, because the prompt is longer than the context of the server.

These results do not show that prompts in STE make this model better or worse on this suite.

The sparkDash score is the mean of the 8 category scores, and not the % of all items. In the score of a category, sparkDash does not count an item with a request error.

## The test

| | |
|---|---|
| Server | the TensorFold engine of `flashnext-fp4local`, version `r3.1-q4mtp`, on one NVIDIA DGX Spark (`spark1`), port 8888 |
| Model | `nvidia/Qwen3.8-Flash-Next-FP4-Local`, revision `b9ce18a6887e18c089fdea8509a33e63429a3300`, with the name `Qwen3.8-Flash-Next-FP4-Local` on the server |
| Bench | sparkDash 2.40.0, Quality bench, `suiteVersion` 1, `scoringVersion` 2 |
| Parameters of the bench | all 8 categories; long context at 8k, 16k, 32k, 64k, 128k and 256k tokens, with 2 items for each size; 8 requests at a time |
| Parameters of each request | `temperature` 0, `seed` 1234 |
| memelint | version 0.0.1, from commit `1fb83bc` of this repository, profile `basic` |
| Baseline | sparkDash bench `eb7b5720-a49e-49cb-9518-6a4cfa8592f2`, label `r3.1`, 2026-10-18 17:41 to 18:29, the same server and parameters, the initial prompts |
| This run | sparkDash bench `88a0529b-6ba3-4165-b30a-850ddbb302b3`, label `memelint`, 2026-10-19 13:20 to 14:16 |

The suite has these categories. sparkDash makes the same items each time. The column `thinking` tells if the model can think before it answers.

| Category | Items | Task | `thinking` | Token limit | Rewritten |
|---|---:|---|---|---:|---:|
| `qa` | 150 | short questions of 9 types: `fixed`, `mul`, `addsub`, `rev`, `count`, `sort`, `date`, `lcm`, `bin` | off | 64 | 0 |
| `reason` | 40 | three persons have objects and give them to each other; then they divide the objects into containers | on | 8192 | 40 |
| `arith` | 40 | a number, and a list of operations to do on it, in sequence | on | 12288 | 40 |
| `track` | 40 | five persons have tokens, and give and find tokens in a long list of steps | on | 12288 | 0 |
| `gsm8k` | 200 | problems from GSM8K, with the answer on a line `Answer: <number>` | on | 8192 | 198 |
| `mmlu` | 285 | questions from MMLU, with four options, and the answer on a line `Answer: <letter>` | off | 1024 | 95 |
| `follow` | 40 | write a short text that obeys 2 or 3 rules about its form; sparkDash examines the rules | off | 1024 | 14 |
| `long` | 12 | find 16 codes in a long text; some codes change after their first value | off | 512 | 0 |

## Method

### 1. Get the suite

[`harness/dump_suite.mjs`](harness/dump_suite.mjs) runs in the sparkDash container. It uses `generateSuite()`, the function of sparkDash that makes the items of the bench, and writes [`results/suite.json`](results/suite.json).

A `long` item has a long text and then an instruction. The long text is data, thus only the instruction is a prompt for memelint. sparkDash makes the long text again for each run.

### 2. Find the problems in each prompt

memelint examined each prompt as a Markdown file, with the profile `basic` (refer to [`results/memelint.toml`](results/memelint.toml)). In this profile, these rules give findings:

| Rule | Finding | Severity |
|---|---|---|
| `1.2` | an approved word in a part of speech in which it is not approved | error |
| `3.5` | the `-ing` form of a verb | error |
| `4.2` | a contraction | error |
| `6.3` | a sentence of more than 25 words | warning |

The profile `strict` also has the rules `1.1` and `1.6`, which examine each word. With `strict`, the suite has 20,317 findings, and each of the 807 prompts has findings. Of these findings, 15,554 are `1.6`: names and technical words that are not in the STE dictionary. A rewrite cannot remove them, and keep the same task. With `basic`, the suite has 647 findings in 408 prompts.

Before memelint examined a prompt, the harness made each protected string into inline code. memelint does not examine code. These are the protected strings:

- each quotation of 1 to 120 characters that has no quotation mark or apostrophe in it, for example `'Answer: <number>'`
- the last phrase of a `follow` rule of the type `ends-with`, and `***`
- the list of names at the end of a `long` instruction, as a block of code

With these protected strings, the suite has 633 findings in 395 prompts.

### 3. The model makes a rewrite of each prompt that has findings

For each prompt with findings, the harness sent a request to the model on port 8888. This is the system prompt:

```text
You are a technical editor. Rewrite the user's text so that it obeys ASD-STE100 Simplified Technical English, as checked by the linter memelint. Fix every memelint finding that is listed. Rules: keep the meaning, the task, every number, name and fact exactly the same; change only the words and sentences that the findings name, plus what is necessary to fix them; split a long sentence into shorter sentences (25 words or fewer each); replace an -ing word with an approved form (for example 'containing' -> 'that contains'); write contractions in full. Copy each protected string character for character. Do not answer, solve or comment on the text. Output only the rewritten text.
```

The user message has the text, each finding with the words where it starts, and the protected strings. The request has `temperature` 0, `seed` 1234, and `thinking` off. Its token limit is 256, plus 0.5 times the number of characters in the text. The harness sent 8 requests at a time.

The harness examined each reply with these checks, the guard. If a reply does not obey one check, the guard rejects it:

1. The reply is not empty.
2. Each protected string occurs in the reply, as many times as in the initial prompt or more.
3. The list of names of a `long` item is at the end, with no change.
4. Each number of the initial prompt occurs in the reply, as many times or more. The guard does not count step labels, for example `(3)`, or the numbers of rules, for example `3. `, because a rewrite can remove them.
5. The reply has no more `Answer:` than the initial prompt. Thus the model did not answer the question.
6. The options of an `mmlu` item are on lines that start with `A. `, `B. `, `C. ` and `D. `, as in the initial prompt.
7. The reply has as many rules with a number as the initial prompt.
8. The reply is 0.5 to 2.5 times as long as the initial prompt.

When the guard rejected a reply, the next request told the model the cause. When the guard accepted a reply, memelint examined it again. The harness stopped when a reply had no findings, or after 4 requests. It kept the reply with the smallest number of findings, but only when that number was smaller than for the initial prompt. If not, it kept the initial prompt.

The rewrite of the suite used 581 s, 447 requests and 61,481 tokens of output. Each reply stopped before its token limit.

### 4. Put the rewritten prompts into the bench

sparkDash sends the prompts, measures the time and gives a score to each answer. The harness does not change sparkDash. A proxy on port 8899 sends each request to port 8888. In each request to `/v1/chat/completions`, it replaced the user message with its rewrite. To find the rewrite, it used the SHA-256 of the text. For a `long` item, it used the SHA-256 of the instruction after the long text. The proxy changed no other field of a request, and no part of a reply.

The harness added port 8899 to the ports of `spark1` in sparkDash, and started the bench with the label `memelint`. After the run, it removed port 8899. The ports of `spark1` are `[8888]` again.

[`results/proxy.jsonl`](results/proxy.jsonl) has one line for each request: 809 requests for the 807 items. The proxy found the record of each request. The 2 more requests are for `long-256k-1` and `long-256k-2`: after an HTTP 400 error, sparkDash sends the request again one time, with different fields for `thinking`.

Before the run, a smoke test sent 6 items through the proxy with `runQualityItem()` of sparkDash: `qa-fixed-01`, `reason-01`, `arith-01`, `gsm8k-0003`, `follow-03` and `long-8k-1`. The proxy found each item, and each answer was correct. Refer to [`results/proxy-smoke.jsonl`](results/proxy-smoke.jsonl).

### 5. Compare with the baseline

The harness compared the two runs item by item, with the item id. It counted the items that only one run answered correctly, and did the exact two-sided McNemar test on them. It also divided the items into two groups: the prompts that the model changed, and the prompts with no change. The second group is a control, because its prompts are the same in the two runs.

## Changes to the prompts

[`changes.md`](changes.md) has each of the 387 changes as a diff, with the findings before and after.

| Rule | Findings before | Findings after |
|---|---:|---:|
| `3.5` (`-ing` form) | 390 | 8 |
| `6.3` (sentence of more than 25 words) | 202 | 12 |
| `4.2` (contraction) | 39 | 0 |
| `1.2` (part of speech) | 2 | 0 |
| Total | 633 | 20 |

| Category | Items | With findings | No findings after | Some findings after | No change | Findings before → after | Rejected replies |
|---|---:|---:|---:|---:|---:|---|---:|
| `qa` | 150 | 0 | | | | 0 → 0 | 0 of 0 |
| `reason` | 40 | 40 | 40 | | | 40 → 0 | 0 of 40 |
| `arith` | 40 | 40 | 40 | | | 40 → 0 | 0 of 40 |
| `track` | 40 | 0 | | | | 0 → 0 | 0 of 0 |
| `gsm8k` | 200 | 200 | 198 | | 2 | 310 → 3 | 8 of 209 |
| `mmlu` | 285 | 101 | 91 | 4 | 6 | 229 → 17 | 0 of 144 |
| `follow` | 40 | 14 | 14 | | | 14 → 0 | 0 of 14 |
| `long` | 12 | 0 | | | | 0 → 0 | 0 of 0 |
| Total | 807 | 395 | 383 | 4 | 8 | 633 → 20 | 8 of 447 |

The harness sent 1 request for 368 prompts, 2 requests for 14 prompts, 3 requests for 1 prompt, and 4 requests for 12 prompts.

The guard rejected 8 replies, all because of a number: 4 for `gsm8k-0835` (the number 3) and 4 for `gsm8k-1215` (the number 15). The harness kept the initial text of these 2 prompts. For 6 `mmlu` prompts, no reply had a smaller number of findings than the initial prompt. Thus the harness kept the initial text of `mmlu-03360`, `mmlu-05676`, `mmlu-05776`, `mmlu-06271`, `mmlu-06860` and `mmlu-10148`.

A rewritten prompt is 0.82 to 1.17 times as long as its initial prompt, with a median of 1.004. In the bench, the 387 rewritten prompts had 191 more prompt tokens than the initial prompts, of 61,640 (0.3 %). For one prompt, the difference was −60 to +47 tokens, with a median of +1.

### Types of change

- `reason` (40): each prompt has a long sentence with `putting any leftover apples in a jar`. The model divided it into short sentences, for example `They put any leftover apples in a jar.`
- `arith` (40): each prompt has one sentence with all the operations, for example `Then, in order: (1) add 3542; (2) replace it by its remainder when divided by 3046, then add 1000; …`. The model made a sentence of each operation. In some rewrites, the step labels are not there, for example `arith-22`.
- `gsm8k` (198): the last line of each prompt is `Solve it step by step. End your reply with a line 'Answer: <number>' containing only the final number.` The model changed `containing` in 198 prompts, but not always with the same words:

  | New text after `'Answer: <number>'` | Prompts |
  |---|---:|
  | `that contains only the final number.` | 105 |
  | `. This line must contain only the final number.` | 36 |
  | `that has only the final number.` | 30 |
  | `. This line contains only the final number.` | 7 |
  | `. This line must have only the final number.` | 3 |
  | `. The line must have only the final number.` | 1 |
  | the line became part of the paragraph of the problem | 16 |

  In 35 prompts, this line was the only change. In 129 prompts, the model also divided sentences of the problem. In 22 prompts, it changed other words of the problem. In 12 prompts, it changed only the spaces between sentences. sparkDash found the `Answer:` line in each reply that stopped before the token limit. Thus the new text of this line did not cause an incorrect result.
- `mmlu` (95): 77 rewrites changed the question or the text before it, and 29 changed the text of an option. 11 rewrites changed a quotation from a source document, for example a statement of 1791 in `mmlu-03295`. Thus the question is about a different text. In 4 rewrites, a long quotation changed: `mmlu-05685`, `mmlu-05743`, `mmlu-05938` and `mmlu-12104`. The guard keeps a quotation only if it has no apostrophe and a maximum of 120 characters.
- `follow` (14): a rule, for example `Use exactly 4 bullet points, each starting with "* ", and no other lists.`, became short sentences, for example `Use exactly 4 bullet points. Each bullet point must start with "* ". Do not use other lists.`

## Results

| Category | This run | Baseline | Rewritten prompts: this run, baseline | Prompts with no change: this run, baseline |
|---|---:|---:|---|---|
| `qa` | 138 / 150 | 138 / 150 | 0 items | 150 items: 138, 138 |
| `reason` | 40 / 40 | 40 / 40 | 40 items: 40, 40 | 0 items |
| `arith` | 40 / 40 | 40 / 40 | 40 items: 40, 40 | 0 items |
| `track` | 40 / 40 | 40 / 40 | 0 items | 40 items: 40, 40 |
| `gsm8k` | 194 / 200 | 195 / 200 | 198 items: 193, 194 | 2 items: 1, 1 |
| `mmlu` | 257 / 285 | 258 / 285 | 95 items: 86, 87 | 190 items: 171, 171 |
| `follow` | 38 / 40 | 39 / 40 | 14 items: 13, 14 | 26 items: 25, 25 |
| `long` | 10 / 12 | 10 / 12 | 0 items | 12 items: 10, 10 |
| Total | 757 / 807 | 760 / 807 | 387 items: 372, 375 | 420 items: 385, 385 |

| Group | Correct only in this run | Correct only in the baseline | McNemar p |
|---|---:|---:|---:|
| All items | 4 | 7 | 0.549 |
| Rewritten prompts | 4 | 7 | 0.549 |
| Prompts with no change | 0 | 0 | 1.000 |

### Items with a different result

We read each of the 11 prompts and the two answers. The guard examines only the form of a rewrite, thus we also looked for changes of meaning.

| Item | Correct in | Change to the prompt | Baseline | This run |
|---|---|---|---|---|
| `gsm8k-0782` | baseline | `thrice a day` became `three times a day`, and one sentence became two; the meaning is the same | 54, correct | 27: the model did not multiply by 2 for the two persons |
| `gsm8k-1035` | baseline | one long sentence became six short sentences; the meaning is the same | 35, correct | 40: 8 movements of 5 s, where the baseline has 7 |
| `gsm8k-1176` | this run | long sentences became short sentences; `need` became `needed` | no answer at the token limit (8192) | 2, correct, after 6719 tokens |
| `mmlu-01038` | baseline | `(without ¬)` became the sentence `Do not use ¬.` | A, correct | no answer at the token limit (1024) |
| `mmlu-02579` | this run | **the meaning changed**: the question became a statement with a cause, `This is because it has many people of different races, ethnic groups, and nationalities.` | C | D, correct |
| `mmlu-02598` | baseline | **the meaning changed**: `algorithmic decision making` became `algorithmic decision`, to remove the `-ing` form | C, correct | B |
| `mmlu-03295` | baseline | the source document of 1791 became short sentences in simple words; 4 of 5 findings removed | B, correct | A |
| `mmlu-10640` | this run | **the meaning of option D changed**: `there is no rule of law that gives the victim …` became `there is no rule of law. It gives the victim …` | no answer at the token limit (1024) | C, correct |
| `mmlu-12104` | this run | in a quotation, `don't` became `do not` | no answer at the token limit (1024) | C, correct |
| `mmlu-12225` | baseline | option B `Adjusting her medication regimen` became `Adjust her medication regimen` | B, correct | no answer at the token limit (1024) |
| `follow-33` | baseline | rule 3 became three sentences; the meaning is the same | 3 of 3 rules | 2 of 3 rules |

- The token limit: `gsm8k-1176`, `mmlu-10640` and `mmlu-12104` stopped at the limit in the baseline, and not in this run. `mmlu-01038` and `mmlu-12225` stopped at the limit in this run, and not in the baseline. In `mmlu`, 12 items stopped at the limit in this run, and 10 in the baseline. In `gsm8k`, 2 items stopped at the limit in this run, and 3 in the baseline.
- A change of meaning: `mmlu-02579`, `mmlu-02598` and `mmlu-10640`. Two of them are correct only in this run, and one only in the baseline.
- The 11 rewrites of a source document gave 10 correct answers. The baseline gave 11.
- The 29 rewrites that changed an option gave 26 correct answers, as in the baseline. Two items had a different result: `mmlu-10640` is correct only in this run, and `mmlu-12225` only in the baseline.

### The same prompt gives the same answer

sparkDash records the SHA-256 of the answer text of each item, without the field `reasoning`. For each of the 420 prompts with no change, the answer text was the same in the two runs. These are 418 answers, and the 2 request errors with no answer. The two runs were on different dates, and only this run went through the proxy.

A rewrite changed the answer text of most items, also when the result did not change. For the rewritten prompts, only 60 of 387 answers are the same as in the baseline:

| Category | Same answer text | Rewritten prompts |
|---|---:|---:|
| `reason` | 1 | 40 |
| `arith` | 14 | 40 |
| `gsm8k` | 43 | 198 |
| `mmlu` | 0 | 95 |
| `follow` | 2 | 14 |

### Tokens and time

- Tokens of output: for the rewritten prompts, the model wrote 188,349 tokens in this run, and 183,481 in the baseline (+2.7 %). In `reason`, the mean was 682 tokens in this run, and 561 in the baseline (+22 %). For the prompts with no change, the output was the same: 165,292 tokens.
- Time: this run used 56.0 min, and the baseline 48.5 min. For the prompts with no change, the answers were the same. But the sum of their times was 10,166 s in this run, and 8,733 s in the baseline (+16 %). The increase was largest for short requests: `qa` +50 %, `follow` +35 %, `mmlu` +31 %, `track` +3 %, `long` +1.5 %. We did not find the cause. Possible causes are the proxy, and a different load on the machine. The time is not part of the score.

### Request errors

`long-256k-1` and `long-256k-2` had an HTTP 400 error in the two runs. Their prompts have 314,050 and 314,100 tokens, but the context of the server is 262,144 tokens. sparkDash does not count these 2 items in the score of `long`.

## Limits

- This is one model on one server, with one run for each set of prompts. At `temperature` 0, the server gives the same output for the same prompt. Thus each difference comes from the text of the prompts.
- A different rewrite of the same prompt can give a different result. The 11 items with a different result show that a small change of text can change the answer of this model.
- The same model wrote the rewrites and answered the prompts. A different editor can make different rewrites.
- The guard examines numbers, protected strings, options and rules. It does not examine the meaning. We read the 11 items with a different result, and 3 of them have a change of meaning. We did not read the other 376 rewrites. [`changes.md`](changes.md) has all of them.
- The guard does not keep a quotation of more than 120 characters, or a quotation with an apostrophe. A rewrite changed such a quotation in 4 `mmlu` prompts.
- A quotation from a source document is part of the question. When the rewrite changes it, the question is different.
- The profile `basic` does not examine words. With `strict`, a project must have a word list for its technical names. Also, a rewrite of a name can change the task.

## Files

| File | Contents |
|---|---|
| [`changes.md`](changes.md) | each rewritten prompt as a diff, with its findings |
| [`changes.py`](changes.py) | writes `changes.md` from `results/corrections.json` |
| [`harness/memelint_suite.py`](harness/memelint_suite.py) | the harness: `correct` makes the rewrites, `serve` is the proxy, `bench` does the bench run and compares it with the baseline |
| [`harness/dump_suite.mjs`](harness/dump_suite.mjs) | writes the suite of sparkDash as JSON |
| [`results/suite.json`](results/suite.json) | the 807 items as sparkDash made them; for a `long` item, only the instruction |
| [`results/memelint.toml`](results/memelint.toml) | the memelint configuration for the prompts |
| [`results/corrections.json`](results/corrections.json) | the record of each item. It has the initial prompt, the protected strings and the findings. It also has each request to the model and its reply, the status, and the text for the bench with its findings |
| [`results/quality-1.json`](results/quality-1.json) | the result of this run from sparkDash, bench `88a0529b` |
| [`results/baseline-quality.json`](results/baseline-quality.json) | the result of the baseline from sparkDash, bench `eb7b5720` |
| [`results/compare.txt`](results/compare.txt) | the table that the harness wrote at the end of the run |
| [`results/proxy.jsonl`](results/proxy.jsonl) | one line for each request through the proxy |
| [`results/proxy-smoke.jsonl`](results/proxy-smoke.jsonl) | the smoke test before the run |
| [`results/memelint-correct.log`](results/memelint-correct.log), [`results/memelint-bench.log`](results/memelint-bench.log) | the output of `correct` and `bench` |
| [`results/index-line.jsonl`](results/index-line.jsonl) | the line that `bench` added to the list of runs of `flashnext-fp4local` |

The files in `harness/` and `results/` are the same, byte for byte, as the files of the run. Thus memelint does not examine them (refer to `ignore` in [`memelint.toml`](../../memelint.toml)). These are the SHA-256 values of the programs:

```text
16646cf42ee3c14dcce1a38bd897c038767ee2d0d0d07f43ec4ec4f2f29653c6  harness/memelint_suite.py
bb190cccaa7dad42ff4461248f35c79c21a31824d05a101ed99bc6639d55cf6d  harness/dump_suite.mjs
```

## Do the run again

For this run, the harness was in a copy of the repository `flashnext-fp4local`, with `memelint_suite.py` in `tools/` and `dump_suite.mjs` in `tools/quality/`. It writes to `results/<label>/` in that copy. It is necessary to have:

- sparkDash at `http://127.0.0.1:5555`, in a Docker container with the name `sparkDash`, with a spark `spark1`
- the server on port 8888, and no other program on port 8899
- a memelint binary, given with `--memelint` or the environment variable `MEMELINT`

```sh
python3 -I tools/memelint_suite.py correct memelint
python3 -I tools/memelint_suite.py bench memelint --port 8899 --concurrency 8 \
  --baseline eb7b5720-a49e-49cb-9518-6a4cfa8592f2
```

`correct` runs for approximately 10 min, and `bench` for approximately 1 h.
