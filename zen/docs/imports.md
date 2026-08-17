# Imports and Packages in Zen

```zen
import "file.z"               // load file
import mypackage              // load from ~/.zen/modules/ or ./
from mypackage import func    // import specific name
import mypackage as mp         // alias
from mypackage import f as x  // aliased import
import mypackage.sub         // dotted import
import /abs/path/file.z      // absolute path
```

Package resolution: `import pkg` looks for `pkg.z`, `pkg/main.z` or `pkg/pkg.z`. Dotted paths (`pkg.sub.mod`) resolve each segment.

`errors.define()` for custom errors:
```zen
errors.define("MyError", "Error", "message")
throw new MyError("details")
catch MyError as e { print e }
```

PM commands:
```bash
zen pm init [name]
zen pm install owner/repo
zen pm install file.z
zen pm install ./dir/
zen pm list
zen pm remove pkg
```
