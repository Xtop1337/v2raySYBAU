# Third-party notices / Сторонние компоненты

V2Ray Sybau itself is licensed under the MIT License (see `LICENSE`).
The installer also ships the third-party programs and data listed below. They are separate
works under their own licenses: they are distributed unmodified, run as separate processes
(Xray-core, sing-box) or are read as data files, and are not part of the application's code.

Код V2Ray Sybau распространяется по лицензии MIT (файл `LICENSE`). Установщик также содержит
перечисленные ниже сторонние программы и данные. Это отдельные произведения со своими лицензиями:
они поставляются без изменений, запускаются как самостоятельные процессы (Xray-core, sing-box)
или читаются как файлы данных и не являются частью кода приложения.

The full license texts are installed next to the application in `resources/licenses`
(V2Ray Sybau → About → Licenses). `SOURCES.txt` there lists the exact versions shipped and
where their source code is.
Полные тексты лицензий установлены вместе с приложением в папке `resources/licenses`
(V2Ray Sybau → О программе → Лицензии). Файл `SOURCES.txt` там содержит точные версии и ссылки
на исходный код.

## Xray-core (`xray.exe`)
- License: Mozilla Public License 2.0 — `MPL-2.0.txt`
- Copyright: Project X (XTLS) and contributors
- Source code: https://github.com/XTLS/Xray-core (the exact tag is in `SOURCES.txt`)

## sing-box (`sing-box.exe`)
- License: GNU General Public License v3.0 or later — `GPL-3.0.txt`, `sing-box.LICENSE.txt`
- Copyright (C) 2022 by nekohasekai
- Source code: https://github.com/SagerNet/sing-box (the exact tag is in `SOURCES.txt`).
  The complete source code of the shipped version is also attached to every
  V2Ray Sybau release: https://github.com/Xtop1337/v2raySYBAU/releases
  (`sing-box-<version>-source.tar.gz`).
- V2Ray Sybau is not a derivative work of sing-box and is not associated with or endorsed by
  the sing-box project.

## Wintun (`wintun.dll`)
- License: Wintun Prebuilt Binaries License — `wintun.LICENSE.txt`
- Copyright (C) WireGuard LLC. Distributed unmodified. https://www.wintun.net

## geoip.dat / geosite.dat
- Built by Loyalsoldier/v2ray-rules-dat: https://github.com/Loyalsoldier/v2ray-rules-dat
  (GNU General Public License v3.0 — `GPL-3.0.txt`; the exact release is in `SOURCES.txt`,
  and its source is attached to every V2Ray Sybau release as
  `v2ray-rules-dat-<version>-source.tar.gz`).
- GeoIP data from Loyalsoldier/geoip (https://github.com/Loyalsoldier/geoip,
  Creative Commons Attribution-ShareAlike 4.0 — `CC-BY-SA-4.0.txt`).
  This product includes GeoLite2 data created by MaxMind, available from https://www.maxmind.com.
- Domain lists based on v2fly/domain-list-community (MIT): https://github.com/v2fly/domain-list-community

## Twemoji Country Flags font (`TwemojiCountryFlags.woff2`, inside the app)
- Font packaging: country-flag-emoji-polyfill by TalkJS, MIT License —
  `country-flag-emoji-polyfill.LICENSE.md`, https://github.com/talkjs/country-flag-emoji-polyfill
- Flag graphics: Twemoji, Copyright Twitter, Inc. and other contributors, licensed under
  CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/). The font is a subset of
  Twemoji Mozilla; no other changes.

## Libraries compiled into the application
The application is built with Rust crates (Tauri, tokio, reqwest, rustls, ring, serde and others)
and JavaScript packages (the Svelte runtime, @tauri-apps/api and plugins), all used unmodified.
They are licensed under permissive licenses — MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0,
BSL-1.0, CDLA-Permissive-2.0 (webpki-roots) — and MPL-2.0 (cssparser, selectors, dtoa-short,
option-ext; their source code is at the URLs listed). None of them is under the GPL. Their names,
versions, licenses, source URLs and license texts are listed in `THIRD-PARTY-DEPENDENCIES.txt`,
generated for every release by `scripts/third_party_licenses.py`.
