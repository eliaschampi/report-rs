# report-rs

Motor PDF determinista. Patrón omr-rs: un manifest JSON entra por stdin,
una respuesta JSON sale por stdout, stderr solo diagnósticos, exit 0 = lote
corrido (errores por ítem permitidos), exit 2 = manifest inservible.

## Estado

**Spike / R0-R1 (payment_ticket)** — demostrador de viabilidad con métricas
medidas. Falta: kinds A4, carnets, integración en coedula (`report-engine.service.ts`),
CI, tests de contrato end-to-end.

```
~/Documents/report-rs
├── assets/fonts/          LiberationMono-{Regular,Bold}.ttf (embebidas por subset)
├── docs/protocol.md       contrato v1 (borrador congelado para el spike)
└── src/
    ├── main.rs            frontera de proceso, exit codes
    ├── protocol.rs        serde estricto, deny_unknown_fields, echo de id
    ├── text.rs            medición + wrap/fit (el ÚNICO motor de texto)
    └── render/ticket.rs   payment_ticket 80mm, altura variable
```

## Métricas medidas (M1/M2 MacBook, release, rustc 1.98)

| Escenario | p50 | p95 | Nota |
|---|---|---|---|
| Render interno 1 ticket | 1.5 ms | 2.2 ms | steady state, fuentes ya cargadas |
| Spawn completo, 1 ticket | 6.9 ms | 7.6 ms | ciclo de vida entero del proceso, off-event-loop |
| Spawn completo, 16 tickets | 31 ms | 32 ms | 1.9 ms/ticket amortizado |
| Determinismo | bytes idénticos entre corridas | | requisito del patrón |
| Salida | 22.8 KB/ticket | | pdf-lib: 1.9 KB (no embebe fuentes; sin UTF-8) |

Baseline pdf-lib medido el mismo día (código de producción replicado):
ticket p50 0.84 ms in-process (bloquea el event loop); reporte A4 100 filas
p50 **217 ms** in-process (178 ms de ellos son el `embedPng` del membrete
decodificado en JS puro). Ver `docs/parity.md` cuando exista.

## Uso

```sh
cargo build --release
python3 - <<'EOF' | ./target/release/report-rs
{"protocol_version":1,"assets_dir":"assets","documents":[{"id":"t-1","kind":"payment_ticket","out_path":"/tmp/t-1.pdf","payload":{"rows":[{"text":"HOLA","align":"center","bold":true}]}}]}
EOF
```

## Gates (heredados de omr-rs)

`cargo fmt --check` · `cargo clippy --all-targets -- -D warnings` · `cargo test`
