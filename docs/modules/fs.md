# File System (fs)

## File Operations

```
fs.list(".")                    // list directory contents
fs.read("file.txt")             // read text file
fs.write("file.txt", "hello")   // write text file
fs.append("file.txt", "more")   // append to file

fs.read_binary("file.bin")      // read binary file
fs.write_binary("file.bin", data)

fs.exists("path")               // check existence
fs.is_file("path")              // is regular file?
fs.is_dir("path")               // is directory?
fs.size("path")                 // file size in bytes
fs.mtime("path")                // last modified timestamp

fs.mkdir("newdir")              // create directory
fs.mkdirs("a/b/c")              // create nested directories
fs.remove("file.txt")           // delete file
fs.rmdir("emptydir")            // delete empty directory
fs.rmtree("dir")                // delete directory + contents

fs.copy("src", "dst")           // copy file
fs.move("src", "dst")           // move file
fs.rename("old", "new")         // rename file

fs.glob("*.txt")                // glob matching
fs.join("a", "b", "c.txt")      // join path parts → "a/b/c.txt"
fs.basename("/a/b/c.txt")       // → "c.txt"
fs.dirname("/a/b/c.txt")        // → "/a/b"

fs.cwd()                        // current working directory
fs.cd("/tmp")                   // change directory

fs.exec("ls -l")                // run shell command
// returns {returncode: 0, stdout: "...", stderr: ""}
```

## Flat Aliases

```
cwd()                        // current working directory
cd("/tmp")                   // change directory
glob("*.txt")                // glob matching
exec("ls -l")                // run shell command
sh("ls -l")                  // alias for exec
read_binary("file.bin")      // read binary file
```
