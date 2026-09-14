# Fixtures do adaptador lopdf

Copiados em 2026-09-14 de `_lab/lopdf_probe/fixtures/`, experimento registrado
pela ADR 0002. Os testes não importam nem executam código de `_lab`.

| Arquivo | Origem | SHA-256 |
|---|---|---|
| `scan.pdf` | sintético, gerado por lopdf 0.44 | `b9339cbf5423f12b9484a03498bef6872fcabf085b9f17cf84668fd5bb643f96` |
| `textops.pdf` | sintético, gerado por lopdf 0.44 | `ba47c6859126ef7297c3c4fba9bad2fcbe7c6ae6051d5556c76f55b3982a5c17` |
| `typst.pdf` | Typst 0.15.1 | `de7b625244b34fac035f10caafe7b568c6e2989352803af751e1bffb21c3da50` |
| `typst_encrypted.pdf` | `typst.pdf`, AES-256 via qpdf | `4c4dbfa8c755c57add5aba2b59c8a3cbb59a24ed0fb76d9b72d67cd7f5e0c8a5` |
| `typst_xrefstream.pdf` | `typst.pdf`, qpdf com xref/object streams | `bddfcf27439a5afbf1a1659a50c961931d7a9d8aa774b1bc9a37cf49420f881c` |
