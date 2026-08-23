# report-rs

Motor PDF determinista. Patrón omr-rs: un manifest JSON entra por stdin,
una respuesta JSON sale por stdout, stderr solo diagnósticos, exit 0 = lote
corrido (errores por ítem permitidos), exit 2 = manifest inservible.

## Estado

**R2 completo + sonda R3.** Kinds: `payment_ticket` (80 mm), 
`attendance_daily_report` (A4 kernel completo: membrete, summary, tablas
zebra, paginación con header repetido, estado vacío) y `student_card`
(sonda CR80: template + QR vectorial + texto rotado; posiciones de PRUEBA,
el mapeo px→pt real es R3). Falta: `student_attendance_report`,
`student_evaluation_report`, `employee_card`, integración en coedula
(`report-engine.service.ts`), CI, tests de contrato end-to-end.

```
~/Documents/report-rs
├── assets/
│   ├── fonts/             LiberationMono + LiberationSans (Regular/Bold)
│   ├── membrete.png       ← colocar (gitignored, asset de marca)
│   └── card.png           ← colocar (gitignored, asset de marca)
├── docs/protocol.md       contrato v1
└── src/
    ├── main.rs            frontera de proceso, dispatch por kind, exit codes
    ├── protocol.rs        serde estricto, payload tipado por kind, echo de id
    ├── text.rs            medición + wrap/fit + runs de glifos (único motor)
    └── render/
        ├── a4.rs          kernel A4 (puerto de pdf-a4-report.service.ts)
        ├── attendance_daily.rs
        ├── card.rs        sonda CR80
        └── ticket.rs      ticket térmico
```

## Métricas medidas (M1/M2 MacBook, release, rustc 1.98, krilla 0.8.2)

| Documento | pdf-lib (baseline, bloquea event loop) | report-rs render interno | report-rs spawn completo |
|---|---|---|---|
| Ticket 80 mm | p50 0.84 ms | p50 1.5 ms | p50 6.9 ms (off-loop) |
| A4 asistencia, 100 filas + membrete | **p50 217 ms** (178 ms = PNG del membrete en JS) | **p50 7.8 ms** (28×) | 31 ms lote de 4 |
| Carnet CR80 (template + QR + rotado) | sin baseline | p50 2.6 ms (48 ms el 1.º: decodifica template) | 8.9 ms/card amortizado |

- Determinismo: **bytes idénticos entre corridas** (verificado con `cmp`).
- Salida: ticket 22.8 KB / A4-100filas 261 KB (7 págs) / carnet 234 KB —
  fuentes embebidas por subset (UTF-8 completo; pdf-lib no embebe y lanza
  con glifos fuera de WinAnsi).
- Binario: **2.15 MB** release (LTO, strip). Sin rustybuzz: los runs de
  glifos se posicionan con ttf-parser (ver abajo).

## Por qué pesa el binario (y por qué no rustybuzz)

`cargo tree`: krilla (pdf-writer + subsetter + tiny-skia-path + png/zune-jpeg/
gif/webp decoders vía feature `raster-images`) + ttf-parser + serde + qrcodegen.
El feature default `simple-text` de krilla trae **rustybuzz** (shaping
tipográfico completo): lo desactivamos — `default-features = false,
features = ["raster-images"]` — y posicionamos glifos con las mismas métricas
ttf-parser con las que medimos el wrap (modelo idéntico a pdf-lib). Ahorro
medido: 2.72 MB → 2.15 MB (−21 %) aunque el binario ahora hace más cosas.
Traer rustybuzz de vuelta sería 1 línea si algún kind necesita shaping real
(árabe, unión de scripts).

## Uso

```sh
cargo build --release
python3 - <<'EOF' | ./target/release/report-rs
{"protocol_version":1,"assets_dir":"assets","documents":[{"id":"t-1","kind":"payment_ticket","out_path":"/tmp/t-1.pdf","payload":{"rows":[{"text":"HOLA","align":"center","bold":true}]}}]}
EOF
```

## Gates (heredados de omr-rs)

`cargo fmt --check` · `cargo clippy --all-targets -- -D warnings` · `cargo test`
