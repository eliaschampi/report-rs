# Protocolo v1

Un manifest JSON por stdin, una respuesta JSON por stdout. Reglas heredadas
del contrato de omr-rs (docs/consumer.md de omr-rs es la referencia normativa
del patrón de integración del caller).

## Entrada

```jsonc
{
  "protocol_version": 1,                 // obligatorio, exactamente 1
  "assets_dir": "/opt/report-rs/assets", // fonts/ + imágenes de marca
  "documents": [                         // 1..=16, orden preservado
    {
      "id": "a4-5981",                   // opaco, no vacío, sin duplicados, se hace eco
      "kind": "payment_ticket",          // enum cerrado (ver kinds)
      "out_path": "/run/coedula/report/job-x/a4.pdf", // caller dueño del FS
      "payload": { /* tipado por kind, ver abajo */ }
    }
  ]
}
```

Campos desconocidos se rechazan en TODOS los niveles (`deny_unknown_fields`):
manifest, documento y payload.

## Kinds v1

### `payment_ticket` — ticket térmico 80 mm

```jsonc
{ "rows": [ { "text": "TITULO", "bold": true, "size": 9.5, "align": "center",
              "separator": false, "gap_after": 4 } ],
  "meta_title": "…", "meta_author": "…", "meta_subject": "…" }   // opcionales
```
`size` en (0, 200); `gap_after` ≥ 0; `align`: `left|center|right`.
Altura variable: 34 + Σ(12 + gap_after), mínimo 240 (idéntico al TS).

### `attendance_daily_report` — A4 con kernel completo

```jsonc
{ "title": "Reporte de presentes del día",
  "subtitle": "Sede · fecha",
  "summary": [ { "label": "Sede", "value": "…", "color": "text" } ],  // 2 columnas
  "columns": [ { "label": "Código", "width": 44 }, … ],   // Σwidth ≤ 499.28
  "rows": [ [ { "text": "1001" }, { "text": "Nombre", "bold": true, "color": "danger" }, … ] ],
  "empty": { "title": "…", "subtitle": "…" },            // obligatorio si rows=[]
  "letterhead": "membrete.png",                          // PNG en assets_dir, opcional
  "meta_*": "…" }
```
Cada fila tiene exactamente tantas celdas como `columns`. `color` (cerrado):
`text|muted|success|warning|danger|info|accent`. Paginación automática con
header repetido; celdas con wrap (texto vacío → línea "—", igual que el TS).

### `student_card` — CR80 (SONDA, posiciones de prueba)

```jsonc
{ "template": "card.png",        // PNG en assets_dir, opcional (sin él: fondo blanco)
  "full_name": "…", "student_code": "…",
  "document_label": "D.N.I.", "document_value": "…",
  "qr_text": "…",                // QR vectorial Medium ECC
  "meta_*": "…" }
```

## Salida

```jsonc
{ "protocol_version": 1,
  "results": [
    { "id": "a4-5981", "status": "ok", "bytes": 261129, "pages": 7 },
    { "id": "x", "status": "error", "error": { "code": "PAYLOAD_INVALID", "message": "…" } }
  ] }
```

## Códigos de error (enum cerrado)

| Código | Significado |
|---|---|
| `PAYLOAD_INVALID` | payload fuera de contrato (tipo incorrecto, width fuera de rango, filas ≠ columnas…) |
| `RENDER_FAILED` | fallo interno de render |
| `WRITE_FAILED` | no se pudo escribir `out_path` |

## Semántica de proceso

- Exit 0: el lote corrió (aunque haya ítems con error). Exit 2: manifest
  inservible — nada en stdout.
- stdout: exactamente la respuesta JSON + `\n`. Nada más, jamás.
- stderr: `{id}: ok {bytes} bytes in {ms} ms` o `{id}: {CODE} ({ms} ms)`.
- Imágenes referenciadas por nombre dentro de `assets_dir`; decodificadas
  una vez por lote (caché por proceso).
- Binario sin dominio: cero alumnos, cero pagos, cero SQL, cero fechas —
  el caller formatea todo (zero hardcoding).
- Determinismo: misma entrada → mismos bytes de salida (verificado).
- Tipografía: LiberationMono (ticket) y LiberationSans (A4/carnet) embebidas
  por subset — UTF-8 completo, sin límite WinAnsi.
