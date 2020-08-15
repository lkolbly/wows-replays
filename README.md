World of Warships Replay Parser
===============================

This program parses `.wowsreplay` files from the World of Warships game. There are two components - a library which you can use in your own Rust programs to extract packets, as well as a command-line program (`replayshark`) which provides some utilities while providing an example usage of the library.

Installing
==========

This project is written in Rust, so first, [install Rust](https://www.rust-lang.org/learn/get-started). Then, clone this repo (or download the zipfile), cd into the root directory, and run:
```
$ cargo build --release
```
After that completes, an executable named `replayshark` should be present in the `target/release` directory:
```
$ ./target/release/replayshark help
```

And away you go!

Dump Utility
============

The utility that you probably want to use (and which works the best) is the `dump` utility, which you can use to convert a `.wowsreplay` file into a `.jl` (JSON lines) file for easier processing by your own tools.

Usage is like:
```
$ ./replayshark dump <my replay file>
```

The first line will be the JSON-encoded meta information from the beginning of the file. The rest of the output will be JSON-encoded packets, containing the following fields you might care about:
- `clock`: The timestamp, in seconds since the game start, of the packet.
- `payload`: The parsed payload.
- `raw`: The raw bytes of the packet, if you can't get what you want from the payload (please feel free to upstream anything you figure out!).

The payload has a single key/value pair, where the key is the type of the object. For example, the `DamageReceived` packet has a payload that might look like:
```
"payload":{"DamageReceived":{"recipient":604965,"damage":[[604981,1254.0]]}}
```
which indicates that the recipient `604965` received 1254 units of damage from `604981`. (you can determine the entity IDs from the "Setup" packet type)

The `wowsreplay` format has a majority of packets under the `7` and `8` packet types, which I refer to as "Entity" packets. These packets have not yet been decoded (their packet ID is unknown), although it is known that they contain an entity ID (that's all that's known about them).

Some packets will appear as "Invalid" packets, these are packets for which the packet ID is known, but for some reason the parser decided it didn't know what to do with the packet.

Trace Utility
=============

The `trace` command requires that you have unpacked the minimap resources into the `res_unpack` folder in your working directory.

Supported Versions
==================

The following game versions are currently supported:
- 0.9.4
- 0.9.5 (and .1)
- 0.9.6 (and .1)
- 0.9.7

The version policy for this component is forward-looking: After game version X is released, I won't work very hard to decode new packets from version X-1 and below. To the extent practical, though, support for older versions will be maintained.

Contributing
============

Feel free to open issues or PRs if you find any bugs or want to be able to parse any particular packets from your replay files. This project is in Rust, but the `dump` command can generate JSON data for consumption in any language, so if you end up writing a packet parser for a new packet in another language please open an issue and I can add it to the Rust code.
