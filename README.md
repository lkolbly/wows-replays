World of Warships Replay Parser
===============================

This program parses `.wowsreplay` files from the World of Warships game. There are two components - a library which you can use in your own Rust programs to extract packets, as well as a command-line program (`replayshark`) which provides some utilities while providing an example usage of the library.

Installing
==========

This project is written in Rust, so first, [install Rust](https://www.rust-lang.org/learn/get-started). Then, clone this repo (or download the zipfile), cd into the root directory. You will need to place a folder `versions/` in the root directory, containing the `scripts/` output unpacked from the game data. For example, `versions/0.10.4/scripts/` should contain an `entities.xml` file.

Then run:
```
$ cargo build --release
```
After that completes, an executable named `replayshark` should be present in the `target/release` directory:
```
$ ./target/release/replayshark help
```

And away you go!

(alternatively, you can download one of the pre-built binaries, which bundles the scripts data for several versions)

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

The payload has a single key/value pair, where the key is the type of the object. For example, the `DamageReceived` packet has a payload that might look like:
```
"DamageReceived": {
    "victim": 576272,
    "aggressors": [
        {
            "aggressor": 576266,
            "damage": 3335.0
        }
    ]
}
```
which indicates that the recipient `576272` received 3335 units of damage from `576266`. (you can determine which the entity IDs map to which players from the "OnArenaStateReceived" packet type)

A majority of the game information is encoded using properties and RPC-style method calls. Properties are set using the "EntityProperty" payload, for example this payload:
```
"EntityProperty": {
    "entity_id": 511260,
    "property": "health",
    "value":62670.0
}
```
indicates that the entity `511260` now has a 62670 health.

Initial values for properties will be set for the object during its `EntityCreate` call.

Additionally, certain properties (in particular properties composed of arrays and/or dictionaries) can be partially updated. For example, the "state" property on the battle state manager (the entity created with type "") can have the team scores updated using this payload:
```
"PropertyUpdate": {
    "entity_id": 511248,
    "property": "state",
    "update_cmd": {
        "levels": [
            {"DictKey": "missions"},
            {"DictKey": "teamsScore"},
            {"ArrayIndex": 0}
        ],
        "action": {
            "SetKey": {
                "key": "score",
                "value": 204
            }
        }
    }
}
```
The "levels" key of the update command indicates the path to update, in this instance the `state["missions"]["teamsScore"][0]` dictionary, updating the `score` key to 204.

Entity methods are encoded using the `EntityMethod` payload.

Some entity method calls have been decoded into an application-specific payload. The `DamageReceived` example above is a packet that originally was a RPC method call but the dump utility converted into a more friendly format.

Some packets will appear as "Invalid" packets, these are packets for which the packet ID is known, but for some reason the parser decided it didn't know what to do with the packet. If you find one of these, please feel free to send me the .wowsreplay file in a new issue!

Supported Versions
==================

Versions 0.9.10 through 0.10.4 have currently been tested.

The version policy for this component is forward-looking: After game version X is released, I won't work very hard to decode new packets from version X-1 and below. To the extent practical, though, support for older versions will be maintained - but it is not guaranteed that any version other than the "current" will work.

The distributed executable contains files extracted from the game, but you can provide your own data files by placing the in the `versions/<version>/` folder in your working directory - for example, `versions/0.10.4/scripts/` should contain the `scripts/` folder unpacked using the [WOWS Unpack Tool](https://forum.worldofwarships.eu/topic/113847-all-wows-unpack-tool-unpack-game-client-resources/).

Acknowledgements
================

Almost all of my understanding of the packet structure comes from [Monstrofil/replays_unpack](https://github.com/Monstrofil/replays_unpack)'s work, and a lot of the parsing code here is rewritten from that code.

Additionally, the framing file format (surrounding the encoded packets) decoding algorithms derive from [evido/wotreplay-parser](https://github.com/evido/wotreplay-parser).

Contributing
============

Feel free to open issues or PRs if you find any bugs or want to be able to parse any particular packets from your replay files. This project is in Rust, but the `dump` command can generate JSON data for consumption in any language, so if you end up writing a packet parser for a new packet in another language please open an issue and I can port it to the Rust code.
