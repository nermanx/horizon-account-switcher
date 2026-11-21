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
| Saját képernyő | `ClientHost::mod_screen_open` + `mod_api::client::widgets` (gombok, szövegmező) -- pontosan a Horizon SDK saját [`widget_screen.rs`](https://github.com/horizon-rust/horizon/blob/main/docs/modding/examples/src/widget_screen.rs) példájának mintáját követve | `src/screen.rs` |
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

Ez a repó **nincs lefordítva**, és a `horizon-mod.toml`-ban lévő `[native]` pin-ek (`horizon`,
`mixins` commit hash-ek) helyőrzők -- ezeket csak a hivatalos eszköz tudja helyesen kitölteni,
mert egy konkrét, aktuális Horizon build commitjához vannak rögzítve:

```sh
# 1. Telepítsd a mod-eszközt (ha még nincs):
cargo install --git https://github.com/horizon-rust/horizon cargo-horizon-mod

# 2. Ebben a mappában (ahol a horizon-mod.toml van) töltsd le a helyes SDK pin-eket:
cargo horizon-mod update
#    (ez kitölti a [native] game/horizon/mixins mezőket a horizon-mod.toml-ban, és lehozza az
#    SDK-t a .horizon/sdk/ alá -- lásd a parancs kimenetét, ha más Horizon buildhez szeretnéd
#    pinelni: cargo horizon-mod update --horizon <build>)

# 3. Build + csomagolás:
cargo horizon-mod build
#    -> target/horizon-mod/account_switcher.hmod

# vagy közvetlen kipróbálás a futó klienssel:
cargo horizon-mod run
```

A forráskódot ebben a session-ben lefordítottam és leteszteltem a Horizon valódi, aktuális
forráskódja ellen (`horizon-mod-api` crate, path-függőségként), `cargo check`, `cargo clippy` és
`cargo test` mind hiba és figyelmeztetés nélkül lefutott -- csak a hivatalos `[native]` pin-ek
hiányoznak, amiket csak a `cargo horizon-mod` eszköz tud helyesen, egy valós, futó Horizon
telepítéssel kitölteni.

### Ismert korlátozás: `--player-dir`

A mod a `horizon-player.txt`-t a folyamat aktuális munkakönyvtárában keresi (ugyanott, ahol az
`options.txt` van). Ha valaki a `--player-dir` kapcsolóval indítja a klienst (ritka, főleg
split-screen esetén), ez a mappa eltérhet attól, amit a motor ténylegesen használ -- ez az egyetlen
eset, amikor a mod és a motor nézete az "aktív név"-ről eltérhet. Ez a kliens futásidejéből
(`--player-dir` érték) nem olvasható vissza egy mod oldaláról.

## Licenc

MIT, lásd [LICENSE](./LICENSE).
