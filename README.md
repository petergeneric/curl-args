ccurl
=====

`ccurl` is a helpful wrapper around `curl` that allows you to automatically add commandline arguments based on the hostname being requested. It's primarily designed for adding `Authorization: Bearer ...` headers, but can also be used for other arguments.


Installing
----------

1. Put the "ccurl" binary on your PATH (either compile yourself, or get the latest version from the Releases tab)
2. Set up a .ccurlrc file in your home folder. An example of this file's contents can be found in `ccurlrc.example.json`.


Compiling
---------
Compile using:
```
cargo build --release
```
