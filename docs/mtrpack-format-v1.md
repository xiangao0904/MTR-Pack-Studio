# MTR Pack Studio Container Format 1

`.mtrpack` is the editable source project format used by MTR Pack Studio. It is a custom little-endian binary container. It is not a ZIP-compatible archive and it does not provide encryption.

## File header

The first 256 bytes are fixed:

| Offset | Size | Value |
| ---: | ---: | --- |
| 0 | 8 | ASCII `MTRPACK\0` |
| 8 | 2 | Container version, currently `1` |
| 10 | 2 | Header size, currently `256` |
| 12 | 4 | Feature flags, currently `0` |
| 16 | 16 | Project UUID |
| 32 | 96 | Checkpoint A |
| 128 | 96 | Checkpoint B |
| 224 | 32 | Reserved, zero-filled |

Each checkpoint contains a generation, index record offset, stored length, raw length, 32-byte BLAKE3 of the raw index, and a 32-byte BLAKE3 of the preceding 64 checkpoint bytes. A zero-filled checkpoint is unused.

## Object records

Records start after the file header and are aligned to 8 bytes. Every record has a 96-byte header:

| Offset | Size | Value |
| ---: | ---: | --- |
| 0 | 4 | ASCII `MOBJ` |
| 4 | 2 | Record version, currently `1` |
| 6 | 2 | Kind: `1` blob, `2` index |
| 8 | 1 | Codec: `0` raw, `1` Zstandard |
| 9 | 1 | Flags, currently `0` |
| 10 | 2 | Record header size, currently `96` |
| 12 | 8 | Raw payload length |
| 20 | 8 | Stored payload length |
| 28 | 32 | BLAKE3 of the raw payload |
| 60 | 32 | BLAKE3 of the stored payload |
| 92 | 4 | Reserved |

The stored payload follows immediately. Zero padding extends the record to an 8-byte boundary. Readers reject unsupported codecs, invalid lengths, out-of-file offsets, or hash mismatches before decoding project data.

## Content index

The current index is a named-field MessagePack document compressed with Zstandard level 3. It records:

- logical schema version and generation;
- previous index offset;
- project UUID, name, description, and target;
- typed content entries;
- BLAKE3 resource identifiers and their record locations.

Physical container versions and logical schema versions evolve independently. A reader must not modify a container or schema version newer than it supports.

## Transactions and recovery

A commit appends changed blobs, synchronizes them, appends and synchronizes a complete index, then writes the inactive checkpoint and performs a full file sync. Readers validate both checkpoints and use the highest valid generation. If the newest checkpoint or index is incomplete, the previous checkpoint remains usable.

Blob identity is the BLAKE3 of its raw bytes. Existing blobs are reused. PNG, JPEG, WebP, AVIF, KTX2, ZIP, and Zstandard payloads remain uncompressed; other blobs use Zstandard only when it saves at least 3 percent.

Compaction writes all live blobs and the current index to a temporary container in the same directory, synchronizes it, and atomically replaces the original. Automatic compaction is eligible when stale data exceeds both 64 MiB and 30 percent of the file.
