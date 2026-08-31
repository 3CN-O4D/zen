# csv — CSV parsing and writing

The `csv` module provides simple tools for reading and writing Comma Separated Values (CSV). It is available globally as `csv`.

```zen
# 1. Parsing a CSV string
var data = "name,age,city\nAda,36,London\nBob,42,Paris"
var rows = csv.parse(data)

for row in rows {
    print(row)  # row is a list: [name, age, city]
}

# 2. Encoding a list of lists back to CSV
var again = csv.encode(rows)
print(again)
```

## Functions

| Function | Description |
|----------|-------------|
| `parse(string)` | Parses a CSV string into a list of lists (rows). |
| `encode(list)` | Encodes a list of lists into a CSV string. |
| `read(path)` | Reads a file and parses its contents as CSV. |
| `write(path, data)` | Encodes a list of lists and writes it to a file. |

## Examples

### Reading a CSV file
If you have a file named `users.csv`:
```zen
var rows = csv.read("users.csv")
var headers = rows.shift() # Get the first row (headers)

for row in rows {
    print("User: ${row[0]}, Age: ${row[1]}")
}
```

### Creating a CSV file
```zen
var data = [
    ["id", "score"],
    [1, 95],
    [2, 88]
]
csv.write("scores.csv", data)
```

## Note on Formatting
`csv.parse` handles standard CSV formatting, including quoted fields and escaped commas.

```zen
var row = csv.parse('Name,"Location, City"')[0]
print(row[1]) # Location, City
```

## See Also
- [fs](fs.md) — For general file operations.
- [json](json.md) — For JSON data handling.
