# Task: write realistic spoken answers, including realistic mistakes, for an independent test set (DS3)

You are helping test an answer checker for a spoken quiz buddy. A learner is asked a question aloud, answers by voice, and speech-to-text turns it into a transcript. The checker must decide, for each **key point** of the model answer, whether the learner **covered** it, **missed** it, or **contradicted** it (said something wrong about it).

The checker was built and tuned on test data written by someone else. Your job is to produce a **fresh, independent** test set, written without seeing that data, so we can learn how the checker behaves on mistakes and phrasings it has never seen. **Independence is the whole point.**

## Files and rules of engagement
- Read **only** `data/ds3_input.json`. It holds 39 system-design quiz questions, each with a model answer and numbered key points (`id` and `text`). Do not read, search for, or ask about any other file in the repository (there is existing test data, "wrong answer" lists and keyword lists; you must not see them).
- Use your own knowledge of system design to decide what real learners get wrong. You may check facts against general knowledge. Do not copy key-point wording into wrong answers; write the way a person talks.
- You are not the author of the original data. Do not try to imitate it.

## What to write
For **every question**, write **at least 6 and at most 8** spoken transcripts. Cover these situations across the set (spread them, do not follow the same order every time):
1. **Correct, own words:** covers every key point but shares as few exact words with the key points as you can.
2. **Correct, noisy speech-to-text:** covers every key point, but the transcript contains typical recognition errors: homophones, split or merged words, numbers spoken as words ("four twenty nine"), missing punctuation, mangled technical terms.
3. **Partial:** covers some key points, silently skips others (label the skipped ones `missing`).
4. **One subtle mistake:** most points right, one point wrong in a way a real learner could plausibly say.
5. **Mixed:** one wrong point **and** one or two missing points, or two wrong points.
6. **Confident wrong with a plausible story:** a mix-up with a neighbouring concept, with a believable reason.
7. (Optional, 0 to 2 per question) **Self-correction:** the learner says something wrong then fixes it ("it's a 403, no wait, 429"). Label by what they finally stand by. Tag these `self_correction`.
8. (Optional) **Hedged wrong:** wrong but unsure ("I think it's maybe the opposite?"). Label `contradicted`.

Aim for about 240 to 300 transcripts in total.

### Mistake design: avoid giveaways
Real mistakes are varied and rarely announce themselves. Across the set, use a wide mix of error types, and record the type for each contradicted point:
`wrong_number`, `swapped_terms` (two related things reversed), `opposite_direction`, `wrong_cause`, `overgeneralization`, `confuses_with_related_concept`, `negation_error`, `partial_truth_with_wrong_detail`, `wrong_order_or_step`, `invented_detail`, `wrong_tradeoff`.
- Do **not** make wrong answers detectable by cue words ("never", "only", "always", "actually"). Vary sentence structure. Put the wrong statement in the middle or the start, not always the end.
- No more than about 20% of the mistakes may be numeric.
- Some mistakes should be **close** to the truth, differing by one word or one detail. Some should be far off. Do not reuse the same mistake across questions.
- A wrong answer must still sound like speech: fillers ("um", "uh", "you know", "like", "I mean"), restarts, trailing off, hedges. Vary the speaker: nervous beginner, confident senior engineer, non-native English speaker, rushed, rambling. Record the persona.

## Labelling rules (strict)
For each key point of the question, give exactly one label:
- `covered`: the transcript conveys the key point's meaning, even reworded, hedged, or in speech-to-text-damaged form.
- `missing`: the transcript does not state it.
- `contradicted`: the transcript states something **incompatible** with the key point.
If a statement is ambiguous (you could argue two labels), **rewrite the transcript** until it is clear. Wrong statements that relate to no key point need no label; avoid them. A transcript may have zero, one or two contradicted points, but every transcript must be unambiguous.

## Output format
One JSON file, `tests/ds3-independent.transcripts.json` (or print it if you cannot write files). Use the same `question_id` and key point `id` values as the input.

```json
{
  "note": "Independent test set DS3. Labels: covered | missing | contradicted.",
  "cases": [
    {
      "id": "c4q1-X01",
      "question_id": "c4q1",
      "persona": "nervous beginner",
      "kind": "one_subtle_mistake",
      "transcript": "Um so, I think it keeps the servers from, like, getting hammered ...",
      "labels": {"k1": "covered", "k2": "contradicted", "k3": "missing"},
      "mistake_types": {"k2": "swapped_terms"},
      "tags": []
    }
  ]
}
```
- `id`: `<question_id>-X<two digits>`, unique.
- `kind`: one of `correct_own_words`, `correct_noisy_stt`, `partial`, `one_subtle_mistake`, `mixed`, `confident_wrong`, `self_correction`, `hedged_wrong`.
- `mistake_types`: only for contradicted points. `tags`: free list, for example `["self_correction", "numeric"]`.
- The example above is format only; it is not a real item and must not be reused.

## Quality checks you must run before you answer
1. The file is valid JSON; every `question_id` exists in the input; every case labels **every** key point of its question, with only the three allowed labels.
2. Every `contradicted` label has a `mistake_types` entry; every transcript with a `mistake_types` entry has that label.
3. No transcript contains a full key-point sentence verbatim, and no two transcripts are near-duplicates.
4. **Blind re-label:** for a random 15% of the cases, cover your labels, re-read only the transcript and key points, and relabel. If your two passes disagree on any point, fix the transcript. Report how many disagreed.
5. Check the balance: the share of `contradicted`, `missing` and `covered` points; the spread of `mistake_types`, `persona` and `kind`; at most 20% numeric mistakes.

## What to report at the end
A short summary: number of questions and cases; counts of labels, kinds, personas and mistake types; results of the blind re-label check; and any questions where you found the key points ambiguous or wrong (list their ids). Do not paste the data into the summary if it is in the file.
