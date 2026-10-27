# Chapter 86: Reference Implementation — Word Frequency Analyzer

Reads a text file, counts word frequencies, and prints the top N most frequent
words. Demonstrates string splitting, dict counting, sorting by value, and file
I/O.

## 86.1 The program (`wordfreq.z`)

```zen
import sys

func clean_word(w) {
    # lowercase and strip surrounding punctuation
    let s = w.lower()
    var start = 0
    var end = len(s)
    while start < end and (s[start] < 'a' or s[start] > 'z') { start += 1 }
    while end > start and (s[end-1] < 'a' or s[end-1] > 'z') { end -= 1 }
    return s[start:end]
}

func word_frequencies(text) {
    let freqs = #{}
    for raw in text.split(" ") {
        let w = clean_word(raw)
        if w != "" {
            freqs[w] = (freqs[w] ?? 0) + 1
        }
    }
    return freqs
}

func read_all(path) {
    with open(path, "r") as f { return f.read() }
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage: zen run wordfreq.z <file> [top_n]")
        return 1
    }
    let top_n = int(argv[2] ?? "10")
    let text = read_all(argv[1])
    let freqs = word_frequencies(text)

    # convert to list of [word, count] and sort by count descending
    let items = []
    for w in freqs { items.push(#(w, freqs[w])) }
    items.sort(fn(a, b) { return b[1] - a[1] })

    print("top " + str(min(top_n, len(items))) + " words:")
    print("========================")
    var i = 0
    while i < top_n and i < len(items) {
        print(str(items[i][0]) + "  " + str(items[i][1]))
        i += 1
    }
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 86.2 Example session

Input `sample.txt`:
```
Zen is a fast language. Zen is simple. Zen is powerful.
```

```
$ zen run wordfreq.z sample.txt 5
top 5 words:
========================
zen  3
is  2
a  1
fast  1
language  1
```

## 86.3 Design notes

- **Word cleaning:** lowercases and strips non-letter characters from the start
  and end of each token (crude; real tokenization handles apostrophes, hyphens,
  etc.).
- **Sorting:** items are sorted by count descending using a comparator closure.
- **Ties:** words with equal counts appear in arbitrary order; add a secondary
  sort by word for determinism.

## 86.4 Exercises

1. Add `--ignore <file>` to load a stop-word list and exclude common words.
2. Add n-gram support (count word pairs instead of single words).
3. Add output as JSON (`--json`).
4. Add case-sensitive mode (`--case-sensitive`).
5. Process multiple files and aggregate counts.
