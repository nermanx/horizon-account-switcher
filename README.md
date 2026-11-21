# Account Switcher — Horizon mod

Natív (Rust) mod a [Horizon](https://playhorizon.dev) Minecraft klienshez
([horizon-rust/horizon](https://github.com/horizon-rust/horizon)): egy gombot tesz a
többjátékos szerverlista képernyőre, amivel elmentett offline becenevek között lehet váltani,
és bármikor egy kattintással vissza lehet térni ahhoz, amivel a mod először elindult.

## Mit csinál -- és mit *nem*

Horizon (ahogy a hivatalos kliens és minden más Minecraft kliens is) csak a **bejelentkezett
Microsoft-fiók** nevét kezeli hitelesítettként: ha be vagy jelentkezve, a kapcsolat mindig a fiók
saját nevét használja, bármit is ír ez a mod a lemezre
([`connection.rs:170-172`](https://github.com/horizon-rust/horizon/blob/main/crates/horizon-client/src/game/network/remote/connection.rs#L170-L172):
*"A signed-in account always plays under its own name"*). Ez a mod **nem tud és nem is próbál**
hitelesített fiókot meghamisítani, szerver oldali bannt vagy anticheatet kijátszani.

Amit a mod ténylegesen csinál: az **offline módú (cracked) szerverek** felé küldött becenevet
cseréli. Ez a név a klienstől kezdve soha nem volt hitelesítve -- maga a motor is explicit módon
szerkeszthetőnek szánja: a `horizon-player.txt` fájlban tárolja, és a saját forráskódja ezt írja
róla: *"An edited name is used as it is"*
([`offline_name.rs`](https://github.com/horizon-rust/horizon/blob/main/crates/horizon-client/src/offline_name.rs)).
A mod csak egy kényelmesebb felületet ad ugyanahhoz, amit eddig is kézzel, egy szövegszerkesztővel
meg lehetett tenni.

## Hogyan működik

| Rész | Mit csinál | Hol |
| --- | --- | --- |
| Gomb a szerverlistán | Egy "Fiókváltás" gomb a `multiplayer` képernyő jobb felső sarkában (`screen_render_post`/`screen_mouse_button_pressed_pre` hook-ok, `event.screen.id == "multiplayer"` -- ez az azonosító stabil, lásd [`multiplayer.rs:202`](https://github.com/horizon-rust/horizon/blob/main/crates/horizon-client/src/multiplayer.rs#L202)) | `src/lib.rs` |
| Saját felület | Egy saját rajzolt overlay a `multiplayer` képernyő felett (`mod_api::client::widgets`: gombok, szövegmező), **nem** a `ClientHost::mod_screen_open` API-val -- az a dokumentációja szerint a címképernyő felett (így a szerverlista felett is) mindig `false`-t ad vissza, nem nyit semmit. A mod ezért saját állapotban (`Option<AccountScreen>`) tartja, hogy nyitva van-e, és minden egeret/billentyűt lenyel ezen a képernyőn, amíg nyitva van -- a widget-ek rajzolása/kezelése pontosan a Horizon SDK saját [`widget_screen.rs`](https://github.com/horizon-rust/horizon/blob/main/docs/modding/examples/src/widget_screen.rs) példájának mintáját követi | `src/lib.rs`, `src/screen.rs` |
| Elmentett profilok | A mod saját, elkülönített tárolójában (`ClientHost::config_files()`, `config/account_switcher/profiles.txt`) -- nem ugyanaz, mint a motor saját `horizon-player.txt` fájlja | `src/profiles.rs` |
| Aktív név váltása | A `horizon-player.txt` felülírása (`name:<érték>\n`), amit a motor minden csatlakozás előtt újraolvas, ha nincs `--name`, `HORIZON_PLAYER_NAME` vagy bejelentkezett fiók ([`launch_options.rs:95`](https://github.com/horizon-rust/horizon/blob/main/crates/horizon-client/src/game/runtime/launch_options.rs#L95)) -- újraindítás nélkül, a következő "Join Server" kattintástól érvényes | `src/player_name.rs` |
| Integritás-ellenőrzés | SHA-256 (saját, függőségmentes implementáció) | `src/sha256.rs` |

## A "hash-elt tárolásról" -- őszintén

Egy Minecraft becenév **sosem titok**: minden más játékos simán látja a szerveren, és a motor is
titkosítatlanul tárolja (`horizon-player.txt`, `servers.dat` játékoslistája, szerverlogok...). Egy
visszafejthetetlen (one-way) hash emiatt két okból sem adna valódi titkosságot:

1. nincs mit titkosítani -- a név eleve nyilvános;
2. működésképtelen lenne: a "visszaváltáshoz" a motornak szó szerint vissza kell kapnia az eredeti
   nevet, amit egy tiszta hash-ből soha nem lehet visszafejteni.

Amit a mod ehelyett ad: minden mentett profilhoz egy SHA-256 ellenőrzőösszeget tárol a saját
mezőiről, és **eldobja** (nem tölti be) azt a bejegyzést, aminek az ellenőrzőösszege nem
egyezik -- vagyis egy kézzel szerkesztett vagy sérült profil-fájlt nem fogad el vakon, ahelyett
hogy érvénytelen adatot használna. Ez integritás-ellenőrzés, nem titkosítás.

Fiók-/jelszó-alapú hitelesítés (pl. több Microsoft-fiók közötti váltás) ezen a módon elvileg sem
valósítható meg biztonságosan egy harmadik féltől származó modból, ezért ez a mod kifejezetten
csak az offline nicknevet kezeli; több Microsoft-fiók közötti váltáshoz a launcher saját,
hivatalos fiókkezelését kell használni.

## Build

A `horizon-mod.toml` `[native]` pin-jei **valódi, érvényes Horizon Nightly commit-hash-ek** (nem
helyőrzők): ezt ebben a session-ben a hivatalos `cargo-horizon-mod` eszközzel (forrásból
telepítve, `cargo install --git https://github.com/horizon-rust/horizon cargo-horizon-mod`)
ellenőriztem, méghozzá a teljes lánccal:

```sh
cargo horizon-mod build   # -> target/horizon-mod/account_switcher.hmod, sikeresen
cargo horizon-mod fits target/horizon-mod/account_switcher.hmod   # -> "made for" (tökéletes illeszkedés)
```

A kapott `.hmod` fájl ténylegesen tartalmazza a `uses.json`-t (a launcher kompatibilitás-ellenőrzése
ezt keresi -- lásd lent), és `cargo check`/`clippy`/`test` is hiba és figyelmeztetés nélkül fut rá.

```sh
# 1. Telepítsd a mod-eszközt, ha még nincs:
cargo install --git https://github.com/horizon-rust/horizon cargo-horizon-mod

# 2. Buildeld a csomagot:
cd horizon-account-switcher
cargo horizon-mod build
#    -> target/horizon-mod/account_switcher.hmod

# vagy közvetlen kipróbálás a futó klienssel:
cargo horizon-mod run
```

A `horizon` Nightly gyakran frissül; ha a pin idővel elavul (a launcher ezt a lenti "fits"
mechanizmussal úgyis jelzi, nem tör el semmit), pinezd újra:

```sh
cargo horizon-mod update                  # a legfrissebb Nightlyre
# vagy egy konkrét buildre:
cargo horizon-mod update --horizon nightly-2026-11-21
```

### Ha mégis a "works on another version only" hibát kapod

Ha ezt látod build/futtatás közben:

> `account_switcher`'s package was made by an older tool and doesn't say what it uses, so it works
> on another Horizon version only.

Ez **nem a mod forráskódjával van baj** -- ezt a saját repómban korábban pont ez okozta, amíg a
`[native]` pin-ek helyőrzők voltak, és a session során meg is találtam + ki is javítottam a
gyökérokot:

1. A launcher a csomagban egy `uses.json` nevű fájlt keres, amit a `cargo horizon-mod build` ír
   bele -- ez sorolja fel, mely API-elemeket (hookokat, típusokat, metódusokat) használja a mod
   kódja, egy adott SDK-hoz fingerprintelve
   ([`launcher/src/native_mods/fit.rs`](https://github.com/horizon-rust/horizon/blob/main/launcher/src/native_mods/fit.rs),
   [`sdk_cmds.rs:249-252`](https://github.com/horizon-rust/horizon/blob/main/crates/cargo-horizon-mod/src/sdk_cmds.rs#L249-L252)).
2. **A `cargo horizon-mod update` megtagadja a futást, ha a meglévő `[native] horizon` pin
   érvénytelen** (pl. egy `0000...0` helyőrző): az `update` a régi pin SDK-ját próbálja először
   lekérni egy összehasonlító riporthoz, és ha az nem létezik, a teljes parancs azonnal hibával
   leáll -- semmi sem frissül
   ([`sdk_cmds.rs: update_report`](https://github.com/horizon-rust/horizon/blob/main/crates/cargo-horizon-mod/src/sdk_cmds.rs#L399):
   `ensure_sdk(root)` a *jelenlegi* pint tölti le, nem az új célt). Ha korábban helyőrző pin volt
   nálad, és lefuttattad az `update`-et, pontosan ez történt -- ezért "nem történt módosítás".

Ha ez a mostani, már valódi pin mellett is előjön: futtasd `cargo horizon-mod build`-et, és ha a
kimenetben megjelenik egy `note: no uses table packed (<ok>)` sor, az pontosan megnevezi az okot
(pl. egy `include!` makró, amit a scanner nem tud elemezni) -- oszd meg velem a zárójeles szöveget.

### Ismert korlátozás: `--player-dir`


A mod a `horizon-player.txt`-t a folyamat aktuális munkakönyvtárában keresi (ugyanott, ahol az
`options.txt` van). Ha valaki a `--player-dir` kapcsolóval indítja a klienst (ritka, főleg
split-screen esetén), ez a mappa eltérhet attól, amit a motor ténylegesen használ -- ez az egyetlen
eset, amikor a mod és a motor nézete az "aktív név"-ről eltérhet. Ez a kliens futásidejéből
(`--player-dir` érték) nem olvasható vissza egy mod oldaláról.

## Licenc

MIT, lásd [LICENSE](./LICENSE).
