"""Cheap exact checks on text: spoken numbers, number mismatch, negation, a few generic antonym pairs.
Pure Python, no dependencies, so it can run on the phone unchanged. Run this file to execute the self-test.
"""
import re

UNITS = {w: i for i, w in enumerate(
    "zero one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen "
    "seventeen eighteen nineteen".split())}
TENS = {"twenty": 20, "thirty": 30, "forty": 40, "fifty": 50, "sixty": 60, "seventy": 70, "eighty": 80, "ninety": 90}
SCALES = {"thousand": 10 ** 3, "million": 10 ** 6, "billion": 10 ** 9, "trillion": 10 ** 12}
NUMWORDS = set(UNITS) | set(TENS) | set(SCALES) | {"hundred"}


def _groups(ch):
    """Number words without hundred/thousand: 'four twenty nine' -> 429, 'sixty two' -> 62, 'four oh three' -> 403."""
    groups, i = [], 0
    while i < len(ch):
        t = ch[i]
        if t in TENS and i + 1 < len(ch) and ch[i + 1] in UNITS and 1 <= UNITS[ch[i + 1]] <= 9:
            groups.append(str(TENS[t] + UNITS[ch[i + 1]]))
            i += 2
        elif t in TENS:
            groups.append(str(TENS[t]))
            i += 1
        else:
            groups.append(str(UNITS[t]))
            i += 1
    return int("".join(groups)) if groups else 0


def _chunk(ch):
    if "hundred" in ch:
        k = ch.index("hundred")
        return (_groups(ch[:k]) or 1) * 100 + _groups(ch[k + 1:])
    return _groups(ch)


def _run_value(run):
    total, chunk = 0, []
    for t in run:
        if t in SCALES:
            total += (_chunk(chunk) or 1) * SCALES[t]
            chunk = []
        else:
            chunk.append(t)
    return total + (_chunk(chunk) if chunk else 0)


def extract_numbers(text):
    """All numbers in a text, from digits or spoken words, as floats."""
    t = text.lower().replace("-", " ")
    t = re.sub(r"(\d[\d,]*(?:\.\d+)?)\s+(thousand|million|billion|trillion)",
               lambda m: str(float(m.group(1).replace(",", "")) * SCALES[m.group(2)]), t)
    toks = re.findall(r"\d[\d,]*(?:\.\d+)?|[a-z']+", t)
    out, i, n = [], 0, len(toks)
    while i < n:
        w = toks[i]
        if w[0].isdigit():
            out.append(float(w.replace(",", "")))
            i += 1
        elif w == "point" and i + 1 < n and toks[i + 1] in UNITS and UNITS[toks[i + 1]] < 10:
            j, digits = i + 1, ""
            while j < n and toks[j] in UNITS and UNITS[toks[j]] < 10:
                digits += str(UNITS[toks[j]])
                j += 1
            out.append(float("0." + digits))
            i = j
        elif w in NUMWORDS:
            run, j = [], i
            while j < n:
                u = toks[j]
                if u in NUMWORDS:
                    run.append(u)
                elif u == "oh" and run and run[-1] in UNITS and j + 1 < n and toks[j + 1] in UNITS and UNITS[toks[j + 1]] < 10:
                    run.append("zero")
                elif u == "and" and run and (run[-1] == "hundred" or run[-1] in SCALES) and j + 1 < n and toks[j + 1] in NUMWORDS:
                    pass
                else:
                    break
                j += 1
            val = float(_run_value(run))
            if j + 1 < n and toks[j] == "point" and toks[j + 1] in UNITS and UNITS[toks[j + 1]] < 10:
                k, digits = j + 1, ""
                while k < n and toks[k] in UNITS and UNITS[toks[k]] < 10:
                    digits += str(UNITS[toks[k]])
                    k += 1
                val += float("0." + digits)
                j = k
            out.append(val)
            i = j
        else:
            i += 1
    return out


def number_info(kp, sent):
    nk = {round(x, 6) for x in extract_numbers(kp)}
    ns = {round(x, 6) for x in extract_numbers(sent)}
    return {"kp_has_num": float(bool(nk)),
            "num_hit": (len(nk & ns) / len(nk)) if nk else 0.0,
            "num_mismatch": float(bool(nk) and bool(ns) and not (nk & ns))}


NEG = {"not", "no", "never", "none", "neither", "nor", "without", "cannot", "nobody", "nothing"}


def neg_count(text):
    return sum(1 for w in re.findall(r"[a-z']+", text.lower()) if w in NEG or w.endswith("n't"))


def neg_flip(kp, sent):
    return float((neg_count(kp) > 0) != (neg_count(sent) > 0))


ANTONYMS = [
    ({"permanent", "permanently"}, {"temporary", "temporarily"}),
    ({"increase", "increases", "more", "greater", "larger", "higher"}, {"decrease", "decreases", "fewer", "less", "lower", "smaller"}),
    ({"stateless"}, {"stateful"}),
    ({"centralized", "central"}, {"decentralized", "local"}),
    ({"always"}, {"never"}),
    ({"strong"}, {"weak"}),
    ({"fast", "faster"}, {"slow", "slower"}),
    ({"synchronous"}, {"asynchronous"}),
]


def antonym_hit(kp, sent):
    kw, sw = set(re.findall(r"[a-z]+", kp.lower())), set(re.findall(r"[a-z]+", sent.lower()))
    for a, b in ANTONYMS:
        if (kw & a and sw & b and not sw & a) or (kw & b and sw & a and not sw & b):
            return 1.0
    clock_kp = bool(re.search(r"(?<!counter )(?<!anti)clockwise", kp.lower()))
    anti_s = bool(re.search(r"anti ?clockwise|counter ?clockwise", sent.lower().replace("-", "")))
    return 1.0 if (clock_kp and anti_s) else 0.0


if __name__ == "__main__":
    cases = {
        "four twenty nine": [429], "four oh three": [403], "five hundred": [500], "sixty two": [62], "forty one bits": [41],
        "three sixty five billion records": [365e9], "eleven sixty writes": [1160], "eleven thousand six hundred": [11600],
        "four thousand ninety six": [4096], "ten thousand": [10000], "a hundred million a day": [1e8],
        "three hundred sixty five": [365], "one hundred and five": [105], "returns 429 too many": [429],
        "11,600 per second": [11600], "100 million urls": [1e8], "point zero zero three percent": [0.003],
        "sixty nine years": [69], "no numbers here, uh-oh": [], "twelve bit sequence": [12],
    }
    bad = 0
    for text, want in cases.items():
        got = extract_numbers(text)
        ok = len(got) == len(want) and all(abs(a - b) < 1e-9 * max(1, abs(b)) for a, b in zip(got, want))
        bad += not ok
        print("ok  " if ok else "FAIL", repr(text), "->", got, "" if ok else f"(wanted {want})")
    ni = number_info("Return HTTP status 429 Too Many Requests.", "um I'd return a four oh three forbidden")
    assert ni["num_mismatch"] == 1.0 and number_info("Return 429", "four twenty nine")["num_mismatch"] == 0.0
    assert neg_flip("It is permanent", "it is not permanent") == 1.0 and antonym_hit("permanent redirect", "a temporary redirect") == 1.0
    assert antonym_hit("go clockwise", "walk counter-clockwise") == 1.0 and antonym_hit("go clockwise", "go clockwise") == 0.0
    print("self-test", "FAILED" if bad else "passed", f"({len(cases)} number cases)")
    raise SystemExit(1 if bad else 0)
