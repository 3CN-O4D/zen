# OS Module (`os`)

Access to the operating system.

```zen
os.platform()              // OS name (linux, macos, windows)
os.name                    // same (constant string)
os.arch()                  // CPU architecture
os.hostname()               // machine hostname
os.pid()                    // current process ID
os.pids()                   // all process IDs
os.cpu_count()              // number of CPUs
os.home()                   // home directory

os.cwd()                    // current working directory
os.chdir("/tmp")            // change directory

os.execute("echo hi")       // {ok: true, code: 0, stdout: "hi\n", stderr: ""}
os.run("echo hi")          // returns stdout; throws on non-zero exit
os.popen("ls")              // alias for execute
os.system("ls")             // returns exit code number

os.env("PATH")              // get env var (null if not set)
os.getenv("PATH")          // alias
os.setenv("K", "V")         // set env var
os.unsetenv("K")            // remove env var
os.args()                   // command-line args list
os.exit(0)                  // exit program
os.kill(1234)               // send SIGTERM to PID
```
