# OS Module (`os`)

Access the operating system. No import needed.

## System Info

```zen
os.platform()              // "linux", "macos", "windows"
os.name                    // same (constant)
os.arch()                  // CPU architecture
os.hostname()               // machine hostname
os.pid()                    // current process ID
os.pids()                   // list of all process IDs
os.cpu_count()              // number of CPUs
os.home()                   // home directory
```

## Environment Variables

```zen
os.env("PATH")              // get env var (null if not set)
os.getenv("PATH")          // alias
os.setenv("K", "V")         // set
os.unsetenv("K")            // remove
```

## File System

```zen
os.cwd()                    // current working directory
os.chdir("/tmp")            // change directory
```

## Running Commands

```zen
// Structured result
let r = os.execute("echo hi")
print r.ok        // true
print r.code      // 0
print r.stdout    // "hi\n"
print r.stderr    // ""

// Return stdout or throw on failure
let out = os.run("echo hello")
print out.strip()   // "hello"

// Return exit code only
let code = os.system("ls")

// Alias for execute
let r = os.popen("ls")
```

## Process Control

```zen
os.args()                   // command-line arguments list
os.exit(0)                  // exit program
os.kill(1234)               // send SIGTERM to PID
```
