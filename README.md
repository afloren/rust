#TensorFlow Rust
[![Version](https://img.shields.io/crates/v/tensorflow.svg)](https://crates.io/crates/tensorflow)
[![Status](https://travis-ci.org/tensorflow/rust.svg?branch=master)](https://travis-ci.org/tensorflow/rust)

TensorFlow Rust provides idiomatic [Rust](https://www.rust-lang.org) language
bindings for [TensorFlow](http://tensorflow.org).

This project is still under active development and not guaranteed to have a
stable API. This is especially true because the TensorFlow C API used by this
project has not yet stabilized.

## Using

If you only wish to use TensorFlow within your own project, simply include 
`tensorflow = "*"` in your `Cargo.toml` file. If the TensorFlow library is 
not already installed on your computer in a place it can be found, you will
need the requirements listed in "Automatically Building Tensorflow" below,
and Cargo will build the library for you.

## Building

If you only intend to use TensorFlow from within Rust, then you don't need to
build TensorFlow manually and can follow the automatic steps. If you do need to
use TensorFlow outside of Rust, the manual steps will provide you with a
TensorFlow header file and shared library that can be used by other languages.

### Automatically building TensorFlow

Install [SWIG](http://www.swig.org) and [NumPy](http://www.numpy.org).  The
version from your distro's package manager should be fine for these two.  Also
install [Bazel](http://bazel.io/docs/install.html), which you may need to do
from source.
Then run `cargo build -j 1`. Since TensorFlow is built during this process, and
the TensorFlow build is very memory intensive, we recommend using the `-j 1`
flag which tells cargo to use only one task, which in turn tells TensorFlow to
build with only on task. Of course, if you have a lot of RAM, you can use a
higher value.
To include the especially unstable API (which is currently the `expr` module),
use `--features tensorflow_unstable`.

### Manually building TensorFlow

Install [TensorFlow from source](https://www.tensorflow.org/versions/master/get_started/os_setup.html#source).
The Python/pip steps are not necessary, but building `tensorflow:libtensorflow.so` is.

In short:

1. Install [SWIG](http://www.swig.org) and [NumPy](http://www.numpy.org).  The
   version from your distro's package manager should be fine for these two.
1. [Install Bazel](http://bazel.io/docs/install.html), which you may need to do
   from source.
1. `git clone --recurse-submodules https://github.com/tensorflow/tensorflow`
1. `cd tensorflow`
1. `./configure`
1. `bazel build -c opt --jobs=1 tensorflow:libtensorflow.so`

   Using --jobs=1 is recommended unless you have a lot of RAM, because
   TensorFlow's build is very memory intensive.

Copy $TENSORFLOW_SRC/bazel-bin/tensorflow/libtensorflow.so to /usr/local/lib.
If this is not possible, add $TENSORFLOW_SRC/bazel-bin/tensorflow to
LD_LIBRARY_PATH.

You may need to run `ldconfig` to reset `ld`'s cache after copying libtensorflow.so.

Now run `cargo build` as usual.
To include the especially unstable API (which is currently the `expr` module),
use `--features tensorflow_unstable`.

## RFCs
RFCs are [issues tagged with RFC](https://github.com/tensorflow/rust/labels/rfc).
Check them out and comment. Discussions are welcome. After all, thats what a Request For Comment is for!

## FAQs

#### Why does the compiler say that parts of the API don't exist?
The especially unstable parts of the API (which is currently the `expr` modul) are
feature-gated behind the feature `tensorflow_unstable` to prevent accidental
use. See http://doc.crates.io/manifest.html#the-features-section.
(We would prefer using an `#[unstable]` attribute, but that
[doesn't exist](https://github.com/rust-lang/rfcs/issues/1491) yet.)

## Other

This project is not directly affiliated with the TensorFlow project, although we
do intend to communicate and cooperate with them.

Developers and users are welcome to join
[#tensorflow-rust](http://chat.mibbit.com/?server=irc.mozilla.org&channel=%23tensorflow-rust)
on irc.mozilla.org.

See [CONTRIBUTING.md](CONTRIBUTING.md) for information on how to contribute code.

This is not an official Google product.

##For more information

* [API docs](https://tensorflow.github.io/rust)
* [TensorFlow website](http://tensorflow.org)
* [TensorFlow GitHub page](https://github.com/tensorflow/tensorflow)
