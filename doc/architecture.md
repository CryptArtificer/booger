# Booger Architecture

## System Overview

```mermaid
graph TB
    subgraph Clients
        A1[AI Agent / Cursor]
        A2[AI Agent / Codex]
        A3[CLI User]
    end

    subgraph "booger (14 MB binary)"
        MCP["MCP Server<br/>JSON-RPC over stdio"]
        CLI["CLI<br/>clap"]
        TOOLS["Tool Dispatch<br/>18 tools"]
        SEARCH["Search Engine"]
        INDEX["Indexer"]
        CTX["Volatile Context"]
        GIT["Git Integration"]
        STORE["SQLite Store"]
    end

    subgraph External
        FS[(Filesystem)]
        OLLAMA[Ollama<br/>Embeddings]
        GITBIN[git binary]
    end

    A1 & A2 -->|stdio| MCP
    A3 --> CLI
    MCP --> TOOLS
    CLI --> TOOLS
    TOOLS --> SEARCH & INDEX & CTX & GIT
    SEARCH --> STORE
    INDEX --> FS
    INDEX --> STORE
    CTX --> STORE
    GIT --> GITBIN
    SEARCH -.->|optional| OLLAMA
    STORE -->|".booger/index.db"| FS

    style MCP fill:#4a9eff,color:#fff
    style TOOLS fill:#ff6b6b,color:#fff
    style STORE fill:#51cf66,color:#fff
    style SEARCH fill:#ffd43b,color:#000
```

## MCP Request Flow

```mermaid
sequenceDiagram
    participant Agent
    participant stdin
    participant run as server::run()
    participant dispatch as dispatch()
    participant handler as handle_tools_call()
    participant call as call_tool()
    participant tool as tool_search()
    participant search as text::search()
    participant store as Store (SQLite)
    participant format as format_results()
    participant stdout

    Agent->>stdin: {"method":"tools/call","name":"search","arguments":{"query":"dispatch"}}
    stdin->>run: read line

    run->>dispatch: route by method
    dispatch->>handler: "tools/call"
    handler->>call: extract name + args
    call->>tool: match "search"

    tool->>search: SearchQuery
    search->>store: FTS5 MATCH query
    store-->>search: ranked chunks
    search->>search: rerank (code boost, focus, visited)
    search-->>tool: Vec<SearchResult>

    tool->>format: results + FormatOpts
    Note over format: inject [note] from annotations
    format-->>tool: formatted string
    tool-->>call: ToolResult::success
    call-->>handler: ToolResult
    handler-->>dispatch: JsonRpcResponse
    dispatch-->>run: Some(response)
    run->>stdout: JSON-RPC response
    stdout-->>Agent: results
```

## Tool Dispatch Map

```mermaid
graph LR
    subgraph "call_tool(name)"
        direction TB
        D{name?}
    end

    subgraph "Search & Discovery"
        T1[search]
        T2[semantic-search]
        T3[hybrid-search]
        T4[grep]
        T5[references]
        T6[symbols]
    end

    subgraph "Indexing"
        T7[index]
        T8[status]
        T9[embed]
    end

    subgraph "Volatile Context"
        T10[annotate]
        T11[annotations]
        T12[focus]
        T13[visit]
        T14[forget]
    end

    subgraph "Git"
        T15[branch-diff]
        T16[draft-commit]
        T17[changelog]
    end

    subgraph "Registry"
        T18[projects]
    end

    D --> T1 & T2 & T3 & T4 & T5 & T6
    D --> T7 & T8 & T9
    D --> T10 & T11 & T12 & T13 & T14
    D --> T15 & T16 & T17
    D --> T18

    style D fill:#ff6b6b,color:#fff
    style T1 fill:#ffd43b,color:#000
    style T2 fill:#ffd43b,color:#000
    style T3 fill:#ffd43b,color:#000
    style T4 fill:#ffd43b,color:#000
    style T5 fill:#ffd43b,color:#000
    style T6 fill:#ffd43b,color:#000
    style T7 fill:#51cf66,color:#000
    style T8 fill:#51cf66,color:#000
    style T9 fill:#51cf66,color:#000
    style T10 fill:#da77f2,color:#fff
    style T11 fill:#da77f2,color:#fff
    style T12 fill:#da77f2,color:#fff
    style T13 fill:#da77f2,color:#fff
    style T14 fill:#da77f2,color:#fff
    style T15 fill:#4a9eff,color:#fff
    style T16 fill:#4a9eff,color:#fff
    style T17 fill:#4a9eff,color:#fff
    style T18 fill:#868e96,color:#fff
```

## Indexing Pipeline

```mermaid
graph LR
    subgraph "1. Walk"
        FS["Filesystem"]
        GI[".gitignore filter"]
        BIN["Binary filter"]
        SIZE["Size filter"]
    end

    subgraph "2. Hash"
        BLAKE3["BLAKE3<br/>content hash"]
        SKIP{"Changed?"}
    end

    subgraph "3. Parse"
        LANG["detect_language()"]
        TS["Tree-sitter<br/>7 languages"]
        RAW["Raw chunk<br/>fallback"]
    end

    subgraph "4. Chunk"
        FN["Functions"]
        ST["Structs/Enums"]
        IM["Imports/Uses"]
        CL["Classes/Traits"]
        SIG["extract_signature()"]
    end

    subgraph "5. Store"
        DB[("SQLite<br/>index.db")]
        FTS["FTS5 index"]
    end

    FS --> GI --> BIN --> SIZE --> BLAKE3
    BLAKE3 --> SKIP
    SKIP -->|"yes"| LANG
    SKIP -->|"no (unchanged)"| DONE["skip"]
    LANG -->|"supported"| TS
    LANG -->|"unknown"| RAW
    TS --> FN & ST & IM & CL
    FN & ST & IM & CL --> SIG
    SIG --> DB
    RAW --> DB
    DB --> FTS

    style TS fill:#ffd43b,color:#000
    style DB fill:#51cf66,color:#000
    style FTS fill:#51cf66,color:#000
    style SIG fill:#ff6b6b,color:#fff
```

## Search Pipeline

```mermaid
graph TB
    Q["Query"]

    subgraph "FTS Search"
        FTS5["SQLite FTS5<br/>tokenize, match"]
        RANK["Base rank<br/>(FTS5 score)"]
        BOOST["Code boost +3<br/>Chunk penalty -4"]
        FOCUS["Focus boost +5"]
        VISIT["Visited penalty -3"]
        ANN["Annotation boost +2"]
    end

    subgraph "Semantic Search"
        EMB_Q["Embed query<br/>(Ollama)"]
        EMB_DB["Load all embeddings"]
        COS["Cosine similarity"]
        TOP_K["Top-K by score"]
    end

    subgraph "Hybrid Search"
        NORM["Normalize scores<br/>FTS: 0→1, Sem: 0→1"]
        MERGE["Merge by (file, line)"]
        ALPHA["alpha × FTS +<br/>(1-alpha) × Semantic"]
        SORT["Sort by hybrid score"]
    end

    subgraph "References"
        ALL["Load all_chunks()"]
        REGEX["\\bsymbol\\b match"]
        CLASS{"Classify"}
        DEF["definition"]
        CALL["call — symbol("]
        TYPE["type — : Symbol"]
        IMP["import"]
        REF["reference"]
    end

    subgraph "Output"
        FMT["format_results()"]
        NOTES["inject [note]<br/>from annotations"]
        MODES["content | signatures<br/>files_with_matches | count"]
    end

    Q --> FTS5 --> RANK --> BOOST --> FOCUS --> VISIT --> ANN
    Q --> EMB_Q --> COS
    EMB_DB --> COS --> TOP_K
    Q --> ALL --> REGEX --> CLASS
    CLASS --> DEF & CALL & TYPE & IMP & REF

    ANN --> FMT
    TOP_K --> FMT
    ANN -->|"FTS results"| NORM
    TOP_K -->|"Sem results"| NORM
    NORM --> MERGE --> ALPHA --> SORT --> FMT
    FMT --> NOTES --> MODES

    style FTS5 fill:#ffd43b,color:#000
    style COS fill:#4a9eff,color:#fff
    style ALPHA fill:#ff6b6b,color:#fff
    style CLASS fill:#da77f2,color:#fff
    style NOTES fill:#51cf66,color:#000
```

## Volatile Context Layer

```mermaid
graph TB
    subgraph "Agent Actions"
        A_ANN["annotate<br/>target + note"]
        A_FOC["focus<br/>paths[]"]
        A_VIS["visit<br/>paths[]"]
        A_FOR["forget<br/>session?"]
    end

    subgraph "SQLite Tables"
        T_ANN[("annotations<br/>target, note, session_id,<br/>created_at, expires_at")]
        T_WS[("workset<br/>path, kind, session_id")]
    end

    subgraph "Effect on Search"
        E_BOOST["Focus paths<br/>rank +5"]
        E_PEN["Visited paths<br/>rank -3"]
        E_ANN_BOOST["Annotation match<br/>rank +2"]
        E_INLINE["[note] in results"]
    end

    subgraph "Session Scoping"
        S1["session: 'abc'<br/>isolated context"]
        S2["session: null<br/>global context"]
    end

    A_ANN -->|add| T_ANN
    A_FOC -->|add focus| T_WS
    A_VIS -->|add visited| T_WS
    A_FOR -->|"session=null: clear ALL<br/>session='x': clear scoped"| T_ANN & T_WS

    T_WS -->|focus| E_BOOST
    T_WS -->|visited| E_PEN
    T_ANN --> E_ANN_BOOST
    T_ANN --> E_INLINE

    S1 & S2 --> T_ANN & T_WS

    style T_ANN fill:#da77f2,color:#fff
    style T_WS fill:#da77f2,color:#fff
    style E_BOOST fill:#51cf66,color:#000
    style E_PEN fill:#ff6b6b,color:#fff
    style E_INLINE fill:#ffd43b,color:#000
```

## Git Integration Flow

```mermaid
graph LR
    subgraph "Input"
        BASE["base branch<br/>(auto-detect:<br/>main or master)"]
        HEAD["current worktree<br/>or staged changes"]
    end

    subgraph "Diff"
        GIT_DIFF["git diff --name-status -z"]
        FILES["changed files"]
    end

    subgraph "Structural Comparison"
        PARSE_BASE["Tree-sitter parse<br/>base version<br/>(git show)"]
        PARSE_HEAD["Tree-sitter parse<br/>head version<br/>(filesystem)"]
        CHUNK_MAP["build_chunk_map()<br/>(kind, name, index)"]
        DIFF["diff_chunks()"]
    end

    subgraph "Output"
        ADDED["Added symbols"]
        MODIFIED["Modified symbols"]
        REMOVED["Removed symbols"]
    end

    subgraph "Consumers"
        BD["branch-diff<br/>JSON summary"]
        DC["draft-commit<br/>commit message"]
        CL["changelog<br/>Markdown"]
        AF["auto-focus<br/>changed files"]
    end

    BASE & HEAD --> GIT_DIFF --> FILES
    FILES --> PARSE_BASE & PARSE_HEAD
    PARSE_BASE & PARSE_HEAD --> CHUNK_MAP --> DIFF
    DIFF --> ADDED & MODIFIED & REMOVED
    ADDED & MODIFIED & REMOVED --> BD & DC & CL
    FILES --> AF

    style DIFF fill:#4a9eff,color:#fff
    style ADDED fill:#51cf66,color:#000
    style MODIFIED fill:#ffd43b,color:#000
    style REMOVED fill:#ff6b6b,color:#fff
```

## Module Dependency Graph

```mermaid
graph TB
    MAIN["main.rs<br/>CLI entry"]
    LIB["lib.rs"]

    subgraph "MCP"
        SERVER["mcp/server.rs"]
        PROTO["mcp/protocol.rs"]
        TOOLS["mcp/tools.rs"]
        RES["mcp/resources.rs"]
    end

    subgraph "Core"
        CONFIG["config.rs"]
        SEARCH_T["search/text.rs"]
        SEARCH_S["search/semantic.rs"]
        INDEX_M["index/mod.rs"]
        CHUNKER["index/chunker.rs"]
        WALKER["index/walker.rs"]
        HASHER["index/hasher.rs"]
    end

    subgraph "Storage"
        SQLITE["store/sqlite.rs"]
        SCHEMA["store/schema.rs"]
    end

    subgraph "Context"
        ANN["context/annotations.rs"]
        WS["context/workset.rs"]
    end

    subgraph "Git"
        GIT_D["git/diff.rs"]
        GIT_F["git/format.rs"]
    end

    subgraph "Embed"
        EMB["embed/mod.rs"]
        OLL["embed/ollama.rs"]
    end

    MAIN --> LIB
    MAIN --> SERVER

    SERVER --> PROTO & TOOLS & RES
    TOOLS --> SEARCH_T & SEARCH_S & INDEX_M & ANN & WS & GIT_D & GIT_F & CONFIG
    TOOLS --> SQLITE

    SEARCH_T --> SQLITE & INDEX_M & CONFIG
    SEARCH_S --> SQLITE & EMB & CONFIG
    INDEX_M --> CHUNKER & WALKER & HASHER & SQLITE & CONFIG

    CHUNKER -->|"Tree-sitter"| TS["7 language<br/>grammars"]
    ANN & WS --> SQLITE
    GIT_D --> CHUNKER
    GIT_F --> GIT_D
    OLL --> EMB
    SQLITE --> SCHEMA

    style MAIN fill:#868e96,color:#fff
    style SERVER fill:#4a9eff,color:#fff
    style TOOLS fill:#ff6b6b,color:#fff
    style SQLITE fill:#51cf66,color:#000
    style CHUNKER fill:#ffd43b,color:#000
    style TS fill:#ffd43b,color:#000
```

## SQLite Schema (v5)

```mermaid
erDiagram
    files {
        int id PK
        text path UK
        text content_hash
        int size_bytes
        text language
        text indexed_at
        text mtime
    }

    chunks {
        int id PK
        int file_id FK
        text kind
        text name
        text signature
        text content
        int start_line
        int end_line
        int start_byte
        int end_byte
    }

    chunks_fts {
        text name
        text content
    }

    embeddings {
        int chunk_id PK
        text model
        blob embedding
    }

    annotations {
        int id PK
        text target
        text note
        text session_id
        text created_at
        text expires_at
    }

    workset {
        int id PK
        text path
        text kind
        text session_id
        text created_at
    }

    files ||--o{ chunks : "has"
    chunks ||--o| embeddings : "may have"
    chunks ||..|| chunks_fts : "synced via triggers"
```
