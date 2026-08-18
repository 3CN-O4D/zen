# Imports and Packages

## Basic Import

```zen
import "greetings.z"          // load file
import greetings              // find greetings.z
import greetings as g         // alias
from greetings import greet   // selective
```

## Package Imports

```zen
import mypackage              // loads mypackage/main.z
import mypackage.utils        // loads mypackage/utils.z
from mypackage.utils import add
```

## Absolute Paths

```zen
import /usr/local/lib/helpers.z
```

## Package Manager

```bash
zen pm init [name]
zen pm install owner/repo
zen pm install file.z
zen pm install ./dir/
zen pm list
zen pm remove pkg
```
