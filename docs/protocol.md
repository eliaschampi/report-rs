# Protocolo v1 (borrador — estado spike)

Un manifest JSON por stdin, una respuesta JSON por stdout. Reglas heredadas
del contrato de omr-rs (docs/consumer.md de omr-rs es la referencia normativa
del patrón de integración del caller).

## Entrada

```jsonc
{
  "protocol_version": 1,               // obligatorio, exactamente 1
  "assets_dir": "/opt/report-rs/assets", // fuentes en assets/fonts/
  "documents": [                        // 1..=16, orden preservado
    {
      "id": "ticket-4821",              // opaco, no vacío, sin duplicados, se hace eco
      "kind": "payment_ticket",         // enum cerrado; más kinds en fases futuras
      "out_path": "/run/coedula/report/job-x/ticket-4821.pdf", // el caller es dueño del FS
      "payload": {
        "rows": [                       // filas declarativas; TODO el contenido vive aquí
          { "text": "I.E. COEDULA", "bold": true, "size": 9.5, "align": "center", "gap_after": 4 },
          { "separator": true }
        ],
        "meta_title": "…", "meta_author": "…", "meta_subject": "…"  // opcionales
      }
    }
  ]
}
```

Campos desconocidos → rechazo (`deny_unknown_fields`). `size` en (0, 200);
`gap_after` ≥ 0; violaciones → error de ítem `PAYLOAD_INVALID`.

## Salida

```jsonc
{ "protocol_version": 1,
  "results": [
    { "id": "ticket-4821", "status": "ok", "bytes": 22840, "pages": 1 },
    { "id": "x", "status": "error", "error": { "code": "WRITE_FAILED", "message": "…" } }
  ] }
```

## Códigos de error (enum cerrado)

| Código | Significado |
|---|---|
| `PAYLOAD_INVALID` | payload fuera de contrato (p. ej. `size` fuera de rango) |
| `RENDER_FAILED` | fallo interno de render (geometría imposible, krilla) |
| `WRITE_FAILED` | no se pudo escribir `out_path` |

## Semántica de proceso

- Exit 0: el lote corrió (aunque haya ítems con error). Exit 2: manifest
  inservible — nada en stdout.
- stdout: exactamente la respuesta JSON + `\n`. Nada más, jamás.
- stderr: una línea por documento con `{id}: ok {bytes} bytes in {ms} ms` o
  `{id}: {CODE} ({ms} ms)`.
- Binario sin dominio: cero alumnos, cero pagos, cero SQL, cero fechas —
  el caller formatea todo (zero hardcoding).
- Determinismo: misma entrada → mismos bytes de salida (verificado).
