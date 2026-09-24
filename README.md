# MyNoSqlServer


### Example of config file
```yaml
PersistenceDest: ~/.mynosqldb/data 
CompressData: true
MaxPayloadSize: 4000000
Location: M1
TableApiKey: 123
InitThreadsAmount: 1
SkipBrokenPartitions: false
SaveThreadsAmount: 2
TcpSendTimeoutSec: 30
BackupFolder: ~/.mynosqldb/backup
BackupIntervalHours: 24
MaxBackupsToKeep: 5
```

### Parameters:
* PersistenceDest - directory where the data is persisted. Every namespace gets a folder of its own inside it (see "Persistence" below);
* CompressData - true/false - enable/disable compression of data between nodes;
* MaxPayloadSize - max size of payload in bytes which is sent to Readers per round trip;
* Location - shows in statusbar of the UI;
* TableApiKey - API key to make irreversible operations with tables through api;
* InitThreadsAmount - amount of threads to initialize data from Storage;
* SkipBrokenPartitions - skip broken partitions during initialization;
* SaveThreadsAmount - amount of threads to save data to Storage;
* TcpSendTimeoutSec - timeout for tcp send operation, otherwise connection will be closed;
* BackupFolder - folder to store backups as ZIP Archives. A snapshot is written into `<name>.zip.tmp` and renamed once it is complete, so a `.zip.tmp` file in the folder is either a backup in progress or the leftover of a process that died mid-archive - never a snapshot to restore from. `/api/Backup/Download` builds its archive in the same folder as `<name>.zip.download.tmp` and removes it when the download ends, so the folder needs room for one more archive while somebody is downloading one;
* BackupIntervalHours - interval between backups;
* MaxBackupsToKeep - max amount of backups to keep per namespace - every namespace has a folder of its own inside BackupFolder and is counted separately. The oldest ones above the limit are deleted by the GcBackups timer, which reports every deletion to the log;



### Persistence

The unit of persistence is a **partition**: the whole partition is serialized to
a JSON array of its rows and compressed with **zstd**. On startup everything is
read into memory, the in-memory tables are rebuilt, and the raw persisted bytes
are released.

`PersistenceDest` is a directory. Every namespace persists into a folder of its
own inside it — the default namespace included:

```text
<PersistenceDest>/default/512, /1024, /tables.meta
<PersistenceDest>/alpha/512,   /1024, /tables.meta
```

#### Slotted page-files

```yaml
PersistenceDest: ~/.mynosqldb/data
```

Data is stored in a set of **size-class page-files** inside the directory
(candle-storage style). Each page-file holds fixed-size slots; the file name is
the slot size in bytes (powers of two starting at 512):

```
<dir>/tables.meta     # table attributes (YAML; legacy JSON still loads), rewritten atomically on change
<dir>/512             # page-file: array of 512-byte slots
<dir>/1024
...
```

A partition is written into the smallest size class its compressed payload fits
into. As long as it keeps fitting the same class it is **overwritten in place**
(no reallocation); if it outgrows the class it moves to a larger one and the old
slot is freed for reuse. Each slot is self-describing (carries its table +
partition key, and `body_len == 0` marks a freed slot) and carries a `crc32`,
so recovery is a plain scan of the page-files — there is no separate on-disk
key index and no persisted free-list. A slot with a failing crc (a torn write)
is skipped on recovery (honouring `SkipBrokenPartitions`).

> There is no automatic conversion between the two formats. To move data from
> one backend to another, take a backup and restore it into a server configured
> with the target `PersistenceDest` — the restore path re-persists everything in
> the new format.


### Write operations and the `TimeStamp` field

For almost every write operation the server **assigns the `TimeStamp` itself** (its
own clock at the moment of the write) and ignores whatever `TimeStamp` the client
sent. This holds for `Insert`, `InsertOrReplace`, `Bulk/InsertOrReplace`,
`Bulk/CleanAndBulkInsert*`, transactions, etc. Here `TimeStamp` means "when the
server stored the row".

#### `InsertOrReplaceIfNew` — conditional upsert by client `TimeStamp`

There is one family of operations where `TimeStamp` means something different — the
**version of the object in a distributed system**, assigned by the *client*:

| Endpoint | Shape |
|----------|-------|
| `POST /api/Row/InsertOrReplaceIfNew` | single entity |
| `POST /api/Bulk/InsertOrReplaceIfNew` | array of entities |
| `POST /api/Bulk/InsertOrReplaceIfNewByChunks` | accumulate chunks, then `...Commit` / `...Cancel` |

Semantics: a row is written **only** when it is missing, or when the incoming
`TimeStamp` is **strictly greater** than the `TimeStamp` already stored. Equal or
older timestamps are silently skipped. This is a last-writer-wins upsert keyed on a
client-owned version, used to converge replicas that may deliver the same object out
of order.

Because the timestamp *is* the version, it is **mandatory** here: every entity must
carry a valid ISO-8601 `TimeStamp`. An entity with a missing or unparseable
`TimeStamp` makes the whole request fail with **HTTP 400** naming the offender:

```
Entity with PartitionKey '<pk>' RowKey '<rk>' does not contain TimeStamp
```

(For the chunked flow this validation runs per chunk at upload time, so a bad chunk
is rejected before the commit.) The response of the successful bulk / chunked
operations is an empty `202`; the single one returns `200`.

Server-clock substitution (the default of every *other* write) and client-version
comparison (these `IfNew` operations) are two distinct request-parsing paths and
must not be confused.

#### `DeleteIf` — conditional delete by the `TimeStamp` the row was read at

The delete counterpart of optimistic concurrency: the client sends back the
`TimeStamp` it read the row at, and the row is deleted **only when that is still the
`TimeStamp` stored in the table**. A row somebody rewrote in the meantime is left
alone.

| Endpoint | Shape |
|----------|-------|
| `DELETE /api/Row/DeleteIf` | single row, `tableName` / `partitionKey` / `rowKey` / `timeStamp` query parameters |
| `POST /api/Bulk/DeleteIf` | array of `{PartitionKey, RowKey, TimeStamp}` in the body |

Here `TimeStamp` is not a version the client owns (as it is for `InsertOrReplaceIfNew`)
— it is the server-assigned write moment of the row, echoed back as an *expected*
value. Comparison is between parsed moments, never between texts, so it does not
matter how many fractional digits the value is spelled with: `...39.5404`,
`...39.540400` and `...39.540400Z` are the same version.

The two shapes differ in how they answer a row that does not match:

- **single** — `200` with the deleted row; `404` when there is no such row; **`409`**
  (`Record is changed`) when the row is there but at another version. Same answers as
  `PUT /api/Row/Replace`.
- **bulk** — always `200`, partial success. The matching rows are deleted, the rest are
  left in place and listed in the response:

```json
{
  "deleted": 2,
  "skipped": [
    { "PartitionKey": "pk1", "RowKey": "rk2", "Reason": "TimeStampMismatch" },
    { "PartitionKey": "pk1", "RowKey": "rk9", "Reason": "NotFound" }
  ]
}
```

An unreadable `timeStamp` / `TimeStamp` fails the request with **HTTP 400** (the bulk
one names the offending row) — a version that cannot be parsed can never match a
stored one, so reporting it as a mere conflict would hide a client bug.

#### `useTimestamp=true` — keep client timestamps on the plain bulk writes

The **unconditional** bulk writes take an optional `useTimestamp` query flag:

- `POST /api/Bulk/InsertOrReplace`
- `POST /api/Bulk/CleanAndBulkInsert`
- `POST /api/Bulk/CleanAndBulkInsertByChunks` (applied per uploaded chunk)

Behaviour of the flag:

- **absent / `false`** (default): the historical behaviour — the server assigns its
  own clock to every row and ignores whatever `TimeStamp` the client sent.
- **`true`**: each row keeps the `TimeStamp` that came in the entity. These stay plain
  **unconditional** operations (unlike `InsertOrReplaceIfNew` there is *no* "is it
  newer" check — every row is written / the clean happens regardless); the flag only
  controls *which* `TimeStamp` gets stored. As with the `IfNew` family the timestamp
  is then mandatory: any entity with a missing or unparseable `TimeStamp` fails the
  request with the same **HTTP 400** naming the offender (for the chunked flow this is
  enforced per chunk at upload, before the commit). The chunked `...Commit` /
  `...Cancel` calls carry no body and therefore no `useTimestamp`.


