# BitWHIP

[![License][license-image]][license-url]
[![Discord][discord-image]][discord-invite-url]

- [What is BitWHIP](#what-is-bitwhip)
- [Using](#using)

## What is BitWHIP

BitWHIP is your swiss army knife for low latency screen sharing. With BitWHIP you can publish your desktop with 30ms of latency.
This video can be sent P2P or to a server. You can watch your BitWHIP generated stream in BitWHIP or on your browser, phone, tv 
or even your car!

BitWHIP is built on open protocols so should work pretty much anywhere. BitWHIP uses WHIP so it can be sent to [Broadcast Box][broadcast-box-url], 
Twitch, Cloudflare or any WHIP server of your choice. It should also interop with your favorite tools and libraries like
OBS, FFmpeg or GStreamer.

## Building
BitWHIP uses [just](https://github.com/casey/just) to make installing dependencies and building easier. To build
this project first you install `just` and then execute `install-deps`.

### Install Just
`cargo install just`

### Install dependencies
`just install-deps`

## Using
Now that you have built you have three different paths.

### Play WHIP

Play WHIP starts a local WHIP server that clients can push too. You can use this to push video from BitWHIP
or other WHIP clients like [OBS](https://obsproject.com/) or [GStreamer](https://gstreamer.freedesktop.org/).

```
just run play whip
```

The WHIP client would use a URL of `http://localhost:1337/` and any Bearer Token you like. You can stream to
it via BitWHIP by running `just run stream http://localhost:1337/ bitwhip`.


### Play WHEP

Play WHEP connects to a WHEP server and plays video. Below is an example of pulling from https://b.siobud.com/ with
a Bearer Token of `bitwhip`

```
just run play-whep https://b.siobud.com/api/whep bitwhip
```

After running this open https://b.siobud.com/publish/bitwhip and your video should open in a native player.

### Stream

**Currently only Windows with NVIDIA cards are supported, more to be added**

Stream captures your local desktop and publish via WHIP. To run this you need a URL and a Bearer Token.
Below is an example of pushing to https://b.siobud.com/ with a Bearer Token of `bitwhip`

```
just run stream https://b.siobud.com/api/whip bitwhip
```

[Join the Discord][discord-invite-url] and we are ready to help!

[license-image]: https://img.shields.io/badge/License-MIT-yellow.svg
[license-url]: https://opensource.org/licenses/MIT
[discord-image]: https://img.shields.io/discord/1162823780708651018?logo=discord
[discord-invite-url]: https://discord.gg/An5jjhNUE3
[broadcast-box-url]: https://github.com/glimesh/broadcast-box
