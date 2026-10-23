# Chapter 46: File Processing Tools

File processing is a common task - reading logs, parsing CSV/JSON, transforming data, and batch operations. This chapter covers building practical file processing tools in Zen.

## Reading and Writing Different Formats

### CSV Processing

```zen
import csv

func read_csv(filename) {
    let content = fs.read(filename)
    let lines = content.split("\n")
    let headers = lines[0].split(",")
    let rows = []
    
    let i = 1
    while i < len(lines) {
        if lines[i].trim() == "" {
            i = i + 1
            continue
        }
        let values = lines[i].split(",")
        let row = {}
        let j = 0
        while j < len(headers) {
            row[headers[j].trim()] = values[j].trim()
            j = j + 1
        }
        rows.push(row)
        i = i + 1
    }
    return {"headers": headers, "rows": rows}
}

func write_csv(filename, headers, rows) {
    let content = headers.join(",") + "\n"
    for row in rows {
        let values = []
        for h in headers {
            values.push(row[h] ?? "")
        }
        content = content + values.join(",") + "\n"
    }
    fs.write(filename, content)
}

# Usage
let data = read_csv("users.csv")
for user in data.rows {
    print(user["name"] + " - " + user["email"])
}
```

### JSON Processing

```zen
import json

func process_json_file(input_file, output_file, processor) {
    let content = fs.read(input_file)
    let data = json.parse(content)
    let result = processor(data)
    with open(output_file, "w") as f {
        f.write(json.stringify(result, indent=2))
    }
}

# Example
process_json_file("data.json", "processed.json", fn(data) {
    # Filter and transform
    let result = []
    for item in data {
        if item.active {
            result.push({
                "name": item.name.upper(),
                "email": item.email
            })
        }
    }
    return result
})
```

### XML-like/Text Processing

```zen
func extract_emails_from_file(filename) {
    let content = fs.read(filename)
    let emails = re.findall(r'[\w\.-]+@[\w\.-]+\.\w+', content)
    return emails
}

func count_words(filename) {
    let content = fs.read(filename)
    let words = content.split(" ")
    return len(words)
}
```

## Log File Analyzer

```zen
import re

class LogAnalyzer {
    func init(log_file) {
        self.log_file = log_file
    }
    
    func parse_logs() {
        let content = fs.read(self.log_file)
        let lines = content.split("\n")
        let entries = []
        
        for line in lines {
            if line.trim() == "" {
                continue
            }
            # Parse Apache log format
            let match = re.match(r'(\S+) \S+ \S+ \[([^\]]+)\] "(\S+) (\S+) ([^"]+)" (\d+) (\S+)', line)
            if match {
                entries.push({
                    "ip": match.group(1),
                    "timestamp": match.group(2),
                    "method": match.group(3),
                    "path": match.group(4),
                    "protocol": match.group(5),
                    "status": int(match.group(6)),
                    "size": match.group(7)
                })
            }
        }
        return entries
    }
    
    func count_by_status() {
        let entries = self.parse_logs()
        let counts = {}
        for entry in entries {
            let status = str(entry.status)
            if counts.contains(status) {
                counts[status] = counts[status] + 1
            } else {
                counts[status] = 1
            }
        }
        return counts
    }
    
    func top_ips(limit = 10) {
        let entries = self.parse_logs()
        let ip_counts = {}
        for entry in entries {
            if ip_counts.contains(entry.ip) {
                ip_counts[entry.ip] = ip_counts[entry.ip] + 1
            } else {
                ip_counts[entry.ip] = 1
            }
        }
        # Sort and return top
        let items = ip_counts.items()
        # Simple sort would go here
        return items
    }
}

let analyzer = LogAnalyzer("access.log")
print(analyzer.count_by_status())
```

## Batch File Processor

```zen
import path

func process_directory(dir, pattern, processor) {
    let files = fs.listdir(dir)
    for filename in files {
        if filename.matches(pattern) {
            let full_path = path.join(dir, filename)
            print("Processing: " + full_path)
            processor(full_path)
        }
    }
}

# Rename files
process_directory("./images", "*.jpg", fn(filepath) {
    let new_name = filepath.replace(".jpg", ".jpeg")
    fs.rename(filepath, new_name)
    print("Renamed to " + new_name)
})

# Convert files
process_directory("./data", "*.txt", fn(filepath) {
    let content = fs.read(filepath)
    let upper = content.upper()
    fs.write(filepath + ".upper", upper)
})
```

## Data Transformation Pipeline

```zen
class Pipeline {
    func init(data) {
        self.data = data
    }
    
    func filter(predicate) {
        let result = []
        for item in self.data {
            if predicate(item) {
                result.push(item)
            }
        }
        self.data = result
        return self
    }
    
    func map(transform) {
        let result = []
        for item in self.data {
            result.push(transform(item))
        }
        self.data = result
        return self
    }
    
    func sort(key_func) {
        # Simple bubble sort
        let arr = self.data
        let n = len(arr)
        let i = 0
        while i < n {
            let j = 0
            while j < n - i - 1 {
                if key_func(arr[j]) > key_func(arr[j+1]) {
                    let tmp = arr[j]
                    arr[j] = arr[j+1]
                    arr[j+1] = tmp
                }
                j = j + 1
            }
            i = i + 1
        }
        self.data = arr
        return self
    }
    
    func take(n) {
        self.data = self.data[0:n]
        return self
    }
    
    func collect() {
        return self.data
    }
}

# Usage
let users = read_csv("users.csv").rows
let result = Pipeline(users)
    .filter(fn(u) { return int(u["age"]) > 25 })
    .map(fn(u) { return {"name": u["name"], "email": u["email"]} })
    .sort(fn(u) { return u["name"] })
    .take(10)
    .collect()

print(json.stringify(result))
```

## File Watcher

```zen
func watch_directory(dir, callback) {
    let last_state = {}
    # Initialize
    let files = fs.listdir(dir)
    for f in files {
        last_state[f] = fs.mtime(path.join(dir, f))
    }
    
    while true {
        let current = {}
        let files = fs.listdir(dir)
        for f in files {
            let p = path.join(dir, f)
            current[f] = fs.mtime(p)
        }
        
        # Check for new/modified files
        for f in current.keys() {
            if !last_state.contains(f) {
                callback(f, "created")
            } elif current[f] != last_state[f] {
                callback(f, "modified")
            }
        }
        
        # Check for deleted files
        for f in last_state.keys() {
            if !current.contains(f) {
                callback(f, "deleted")
            }
        }
        
        last_state = current
        time.sleep(1)
    }
}

watch_directory("./watch", fn(filename, event) {
    print(event + ": " + filename)
})
```

## Text File Merger and Splitter

```zen
func merge_files(file_list, output_file) {
    let content = ""
    for f in file_list {
        content = content + fs.read(f) + "\n"
    }
    fs.write(output_file, content)
    print("Merged " + len(file_list) + " files")
}

func split_file(filename, lines_per_file) {
    let content = fs.read(filename)
    let lines = content.split("\n")
    let i = 0
    let part = 0
    while i < len(lines) {
        let chunk = lines[i : i + lines_per_file]
        let out_name = filename + ".part" + part
        fs.write(out_name, chunk.join("\n"))
        print("Created " + out_name)
        i = i + lines_per_file
        part = part + 1
    }
}
```

## Data Validation

```zen
func validate_csv_row(row, rules) {
    for field in rules.keys() {
        if rules[field].required && (!row.contains(field) || row[field] == "") {
            throw "Missing required field: " + field
        }
        if row.contains(field) && rules[field].type == "email" {
            if !row[field].contains("@") {
                throw "Invalid email in " + field
            }
        }
        if row.contains(field) && rules[field].type == "number" {
            try {
                int(row[field])
            } catch {
                throw "Invalid number in " + field
            }
        }
    }
    return true
}

let rules = {
    "email": {"required": true, "type": "email"},
    "age": {"required": true, "type": "number"},
    "name": {"required": true}
}

let data = read_csv("users.csv")
for row in data.rows {
    try {
        validate_csv_row(row, rules)
        print("Valid: " + row["name"])
    } catch (e) {
        print("Invalid: " + e)
    }
}
```

## Chapter Summary

- Process CSV, JSON, and text files
- Build log analyzers and data pipelines
- Create batch processors for directories
- Watch directories for changes
- Merge, split, and transform files
- Validate data during processing
- Use functional patterns with pipelines

**Next:** Chapter 47 - Network Security Tools
