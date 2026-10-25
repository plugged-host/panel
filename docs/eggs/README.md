# Eggs

The eggs Calagopus ships with, in Pelican's `PLCN_v3` export format (the same shape the panel's
Admin › Nests & Eggs import reads). They are also imported into the live dev panel.

| File | Egg | Nest |
|---|---|---|
| `minecraft-java.json` | Minecraft: Java Edition (Purpur, Fabric, Forge, NeoForge) | Minecraft |
| `velocity.json` | Velocity | Minecraft |
| `vanilla-bedrock.json` | Vanilla Bedrock | Minecraft |
| `fivem.json` | FiveM / RedM | FiveM |
| `multi-theft-auto.json` | Multi Theft Auto | Multi Theft Auto |
| `devbox.json` | plugged.host Devbox | Devbox |
| `python.json` | python generic | Software |
| `nodejs.json` | node.js generic | Software |
| `rust.json` | rust generic | Software |
| `golang.json` | golang generic | Software |
| `code-server.json` | Code-Server | Software |

Import a folder into a nest on the panel box (never build there):

```sh
mkdir -p /tmp/software-eggs
cp docs/eggs/{python,nodejs,rust,golang,code-server}.json /tmp/software-eggs/
panel-rs nests mass-import --nest Software /tmp/software-eggs
```

`nests mass-import` scans one folder; keep eggs for different nests in separate folders or pass
`--nest` per import. `--update` (the default) updates an egg with the same UUID or name, and
`--ignore-errors` keeps going past one bad file. Pelican writes `"files": "[]"` for an egg without
config files; the importer accepts it as an empty map (`shared/src/deserialize.rs`).
