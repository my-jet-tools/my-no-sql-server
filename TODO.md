# TODO

## Removed dead code — possibly missing functionality

При очистке dead-code warnings были удалены структуры, которые объявлены, но нигде не использовались. Возможно, под них планировались HTTP-эндпоинты/операции, которые так и не были подключены. Стоит проверить и либо реализовать недостающую функциональность, либо подтвердить что она не нужна.

### 1. `DataReaderChangesResult` — push-обновления для data-reader подписчика
**Файл:** `src/http_server/controllers/data_reader_controller/models.rs`

Контракт ответа для эндпоинта, который должен был отдавать data-reader клиенту изменения по подписанным таблицам:
- `initTables` — полная инициализация таблиц при первом подключении
- `initPartitions` — инициализация отдельных партиций
- `initRows` — инициализация отдельных строк
- `deleteRows` — список удалённых строк (через `DeleteRowsHttpContract`)

**Что проверить:** есть ли HTTP-долгий-poll/SSE эндпоинт `/Changes` для data-reader (по аналогии с TCP push-уведомлениями). Если data-reader работает только по TCP — структура не нужна.

### 2. `DeleteRowsHttpContract` — нотификация об удалённых строках
**Файл:** `src/http_server/controllers/data_reader_controller/models.rs`

Структура `{ pk: String, rk: Vec<String> }` — должна была передавать клиенту список удалённых row-ключей в рамках одной партиции. Использовалась только внутри `DataReaderChangesResult`, удалена вместе с ним.

### 3. `DeletePartitionsModel` — body-модель для bulk-удаления партиций
**Файл:** `src/http_server/controllers/rows_controller/models.rs`

Структура `{ partitionKeys: Vec<String> }` — JSON-body для эндпоинта массового удаления партиций по списку ключей.

**Что проверить:** есть ли HTTP-эндпоинт `DELETE /Partitions` (или аналогичный), принимающий список партиций в body. Сейчас в `rows_controller` есть `partition_keys` только как query-параметр (что ограничено по длине URL) — возможно, body-вариант так и не был реализован.

### 4. `DeleteModel` — модель для удаления rows по ключу/значениям
**Файл:** `src/db_operations/write/replace.rs`

Структура `{ key: String, values: Vec<String> }`. Лежала в модуле `replace`, но семантически относится к delete-операциям. Назначение неясно — возможно, заготовка под bulk-delete-by-secondary-key или отменённый рефакторинг.

**Что проверить:** нужна ли операция удаления rows по составному фильтру (key + множество значений). Если нет — оставить удалённой.
