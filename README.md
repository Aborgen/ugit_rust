# ugit_rust
A clone of git written in Rust, based on the architecture of https://www.leshenko.net/p/ugit

### Implemented command
* `init` -- Creates an empty repository
* `commit -m MESSAGE` -- Creates a new snapshot of the current state of the ugit project with a description
* `log [optional] HASH/REF` -- Prints descending list of commits from HEAD or an optional starting point
* `checkout HASH/REF` -- Sets HEAD to given identifier, and updates the ugit project appropriately
* `tag NAME [optional] HASH/REF` -- Creates an alias NAME pointing at either HEAD or an optional identifier
* `branch NAME [optional] HASH/REF` -- Creates a new branch NAME starting at either HEAD or an optional identifier
* `hash-object FILE` -- Creates a copy of given FILE with the filename set as FILE's SHA2 hash
* `cat-file HASH` -- Prints the contents of a previously hash-object'd file
* `write-tree` -- Creates a snapshot of the ugit project
* `read-tree HASH` -- Replaces the contents of the ugit project with the file state as stored from a previous write-tree operation
