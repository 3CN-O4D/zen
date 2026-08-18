# CSV Module (`csv`)

Read and write CSV files.

```zen
csv.read("data.csv")          // list of lists
csv.write("out.csv", [["Name","Age"],["Ada",36]])
csv.parse("a,b\nc,d")          // parse CSV string
csv.encode([["Name","Age"],["Ada",36]])
```
