# System Modules (`os`, `time`, `datetime`, `math`)

Zen provides a comprehensive set of system-level and mathematical modules. Each is implemented natively in Rust, ensuring fast execution and no external dependencies.

---

## `os` Module (Operating System)

Provides access to the underlying operating system's services.

### Environment
```zen
print os.platform()       // e.g. "linux", "darwin", "windows"
print os.name             // same as platform()
print os.arch()           // CPU architecture
print os.hostname()       // system hostname
print os.pid()            // current process ID
print os.pids()           // list of all process IDs
print os.cpu_count()      // number of CPU cores

let env_val = os.getenv("PATH")
print env_val

os.setenv("MY_VAR", "value")
os.unsetenv("MY_VAR")
```

### File System Interactions (also covered in `fs`)
```zen
print os.cwd()          // current working directory
os.chdir("/tmp")        // change directory
let home = os.home()    // user home directory
```

### Process Control
```zen
os.exit(0)              // terminate process with exit code 0
```

### Running Commands
```zen
// os.execute() — run a command, return structured result
let result = os.execute("echo hello")
print result.ok        // true
print result.code      // 0
print result.stdout    // "hello\n"
print result.stderr    // ""

// os.run() — run a command, return stdout or throw on failure
let output = os.run("echo hello")
print output.strip()   // "hello"

// os.system() — run a command, return exit code
let code = os.system("echo hello")

// os.popen() — alias for os.execute()
let r = os.popen("ls")
```

---

## `time` Module

Provides time and date utilities.

### Current Time
```zen
print time.now()           // UNIX timestamp (f64 seconds since epoch)
print time.unix()          // same as now()
print time.utc()           // UTC timestamp
```

### Formatting & Parsing
```zen
let now_formatted = time.format(time.now(), "%Y-%m-%d %H:%M:%S")
print now_formatted

let parsed = time.parse("2024-01-15", "%Y-%m-%d")
print parsed
```

### Date Components
```zen
let dt = time.date(time.now())
let year = time.year(dt)
let month = time.month(dt)    // 1..12
let day = time.day(dt)
let hour = time.hour(dt)
let minute = time.minute(dt)
let second = time.second(dt)
let weekday = time.weekday(dt)  // 0=Monday .. 6=Sunday

// Convenience: all components at once
let parts = time.parts(time.now())
print parts
```

### Sleep / Wait
```zen
time.sleep(5)         // pause 5 seconds
time.wait(2500)       // pause 2500 milliseconds
```

---

## `datetime` Module (Dates and Calendars)

The `datetime` module provides rich date arithmetic and calendar-aware operations, including weekday constants.

### Current Date & Time
```zen
print datetime.now()        // datetime value
print datetime.utcnow()     // UTC datetime
print datetime.today()      // date-only (no time)
```

### Date Arithmetic
```zen
let tomorrow = datetime.add_days(datetime.now(), 1)
print tomorrow
```

### Weekday Constants
```zen
print datetime.MONDAY   // 0.0
print datetime.TUESDAY  // 1.0
print datetime.WEDNESDAY // 2.0
print datetime.THURSDAY // 3.0
print datetime.FRIDAY   // 4.0
print datetime.SATURDAY // 5.0
print datetime.SUNDAY   // 6.0
```

### Component Extraction
```zen
let d = datetime.today()
let y = datetime.year(d)
let m = datetime.month(d)
let day_num = datetime.day(d)
```

### Examples
```zen
let today = datetime.now()
print today
let iso = datetime.format(today, "%Y-%m-%d")
print iso
let with_plus_7 = datetime.add_days(today, 7)
print with_plus_7
```

---

## `math` Module

Provides comprehensive mathematical functions and constants.

### Constants
```zen
print math.pi          // 3.141592653589793
print math.e           // 2.718281828459045
print math.inf         // Infinity
print math.nan         // NaN
```

### Trigonometric Functions (angles in radians)
```zen
print math.sin(math.pi / 2)    // 1.0
print math.cos(0.0)            // 1.0
print math.tan(math.pi)        // ~0.0
```

### Angle Conversion
```zen
print math.degrees(math.pi)    // 180.0
print math.radians(180)        // pi.0
```

### Hyperbolic & Other
```zen
print math.hypot(3.0, 4.0)     // 5.0
print math.copysign(1.5, -1.0) // -1.5
```

### Logarithms & Exponentials
```zen
print math.exp(1.0)            // e
print math.log(math.e)         // 1.0
print math.log2(8.0)           // 3.0
print math.log10(100.0)        // 2.0
print math.sqrt(16.0)          // 4.0
print math.abs(-5.0)           // 5.0
print math.floor(3.7)          // 3.0
print math.ceil(3.2)           // 4.0
print math.trunc(3.8)          // 3.0
print math.round(3.5)          // 4.0
```

### Number Theory
```zen
print math.gcd(12, 8)          // 2.0
print math.lcm(12, 8)          // 24.0
print math.factorial(5)        // 120.0
print math.comb(5, 2)          // 10.0  (5 choose 2)
print math.perm(5, 2)          // 20.0  (5 permute 2)
print math.remainder(10, 3)    // 1.0
```

### Summation & Statistics over Ranges
```zen
print math.fsum([1, 2, 3, 4])   // 10.0
print math.prod([2, 3, 4])      // 24.0
```

### Decomposition & Reconstruction
```zen
let (f, e) = math.frexp(10.0)
print f, e                       // 0.625, 4
print math.ldexp(0.5, 3)         // 4.0  (0.5 * 2^3)
```