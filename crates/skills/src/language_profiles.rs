//! World-Class Multi-Language Software Engineering Profiles and Repository Detection.
//!
//! Covers all 10 core enterprise languages:
//! 1. Rust
//! 2. TypeScript
//! 3. JavaScript
//! 4. Python
//! 5. C
//! 6. C++
//! 7. C# (.NET)
//! 8. Java (JVM)
//! 9. Go
//! 10. SQL (PostgreSQL, SQLite, ANSI)
//!
//! Incorporates official compiler/language documentation, Google Style Guides,
//! SOTA skills mappings, Superpowers development discipline, and enterprise security standards.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The 10 core languages supported by OSMOO's autonomous engineering system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportedLanguage {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    C,
    Cpp,
    CSharp,
    Java,
    Go,
    Sql,
}

impl SupportedLanguage {
    pub fn all() -> &'static [SupportedLanguage] {
        &[
            SupportedLanguage::Rust,
            SupportedLanguage::TypeScript,
            SupportedLanguage::JavaScript,
            SupportedLanguage::Python,
            SupportedLanguage::C,
            SupportedLanguage::Cpp,
            SupportedLanguage::CSharp,
            SupportedLanguage::Java,
            SupportedLanguage::Go,
            SupportedLanguage::Sql,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SupportedLanguage::Rust => "rust",
            SupportedLanguage::TypeScript => "typescript",
            SupportedLanguage::JavaScript => "javascript",
            SupportedLanguage::Python => "python",
            SupportedLanguage::C => "c",
            SupportedLanguage::Cpp => "cpp",
            SupportedLanguage::CSharp => "csharp",
            SupportedLanguage::Java => "java",
            SupportedLanguage::Go => "go",
            SupportedLanguage::Sql => "sql",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        let lower = s.to_lowercase();
        let trimmed = lower.trim();
        match trimmed {
            "rust" | "rs" => Some(SupportedLanguage::Rust),
            "typescript" | "ts" | "tsx" => Some(SupportedLanguage::TypeScript),
            "javascript" | "js" | "jsx" | "mjs" | "cjs" | "node" => Some(SupportedLanguage::JavaScript),
            "python" | "py" | "py3" => Some(SupportedLanguage::Python),
            "c" => Some(SupportedLanguage::C),
            "cpp" | "c++" | "cxx" | "cc" | "hpp" => Some(SupportedLanguage::Cpp),
            "csharp" | "c#" | "cs" | "dotnet" => Some(SupportedLanguage::CSharp),
            "java" | "jvm" => Some(SupportedLanguage::Java),
            "go" | "golang" => Some(SupportedLanguage::Go),
            "sql" | "postgres" | "postgresql" | "sqlite" | "mysql" => Some(SupportedLanguage::Sql),
            _ => None,
        }
    }
}

/// Comprehensive architectural profile for a language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageProfile {
    pub language: SupportedLanguage,
    pub name: String,
    pub primary_extensions: Vec<String>,
    pub manifest_files: Vec<String>,
    pub baseline_version: String,
    pub official_docs: Vec<String>,
    pub google_style_guide: Option<String>,
    pub sota_skill_id: String,
    pub idiomatic_patterns: Vec<String>,
    pub common_antipatterns: Vec<String>,
    pub type_and_error_handling: String,
    pub concurrency_and_resources: String,
    pub security_considerations: Vec<String>,
    pub linters_and_formatters: Vec<String>,
    pub test_frameworks_and_checks: Vec<String>,
    pub code_review_checklist: Vec<String>,
    pub required_tools: Vec<String>,
}

/// Repository detection evidence for a single language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageEvidence {
    pub language: SupportedLanguage,
    pub file_count: usize,
    pub matched_manifests: Vec<String>,
    pub matched_configs: Vec<String>,
    pub sample_files: Vec<String>,
    pub confidence_score: f32, // 0.0 to 1.0
}

/// Comprehensive multi-language detection result for a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiLanguageDetectionResult {
    pub workspace_root: PathBuf,
    pub total_code_files: usize,
    pub detected_languages: Vec<LanguageEvidence>,
    pub primary_language: Option<SupportedLanguage>,
    pub is_polyglot: bool,
}

impl MultiLanguageDetectionResult {
    pub fn has_language(&self, lang: SupportedLanguage) -> bool {
        self.detected_languages.iter().any(|d| d.language == lang)
    }

    pub fn language_names(&self) -> Vec<String> {
        self.detected_languages
            .iter()
            .map(|d| d.language.as_str().to_string())
            .collect()
    }
}

/// Registry of authoritative language engineering profiles.
pub struct LanguageProfileRegistry {
    profiles: HashMap<SupportedLanguage, LanguageProfile>,
}

impl Default for LanguageProfileRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageProfileRegistry {
    pub fn new() -> Self {
        let mut profiles = HashMap::new();

        // 1. Rust Profile
        profiles.insert(
            SupportedLanguage::Rust,
            LanguageProfile {
                language: SupportedLanguage::Rust,
                name: "Rust".into(),
                primary_extensions: vec!["rs".into()],
                manifest_files: vec!["Cargo.toml".into(), "Cargo.lock".into()],
                baseline_version: "Rust 1.76+ (2021 / 2024 Edition)".into(),
                official_docs: vec![
                    "https://doc.rust-lang.org/book/".into(),
                    "https://doc.rust-lang.org/reference/".into(),
                    "https://doc.rust-lang.org/std/".into(),
                    "https://doc.rust-lang.org/cargo/".into(),
                ],
                google_style_guide: None, // Rust community adheres strictly to rustfmt & API guidelines
                sota_skill_id: "sota-rust".into(),
                idiomatic_patterns: vec![
                    "Strict ownership, borrowing, and RAII resource cleanup".into(),
                    "Explicit error propagation via Result<T, E> and ? operator".into(),
                    "Type-driven development with newtypes and algebraic data types (enums)".into(),
                    "Composition and polymorphism via traits and generics without inheritance".into(),
                    "Cancellation-safe async primitives using tokio::select! with cancellation tokens".into(),
                ],
                common_antipatterns: vec![
                    "Unnecessary .unwrap() or .expect() on user or network data in production".into(),
                    "Indiscriminate use of unsafe blocks without formal safety proof comments".into(),
                    "Holding std::sync::MutexGuard across .await points (causes async deadlocks)".into(),
                    "Excessive .clone() calls masking lifetime or borrow architecture design flaws".into(),
                ],
                type_and_error_handling: "Static, affine type system. No null pointers; Option<T> for presence. Errors represented as values (Result<T, E>). Use thiserror for domain libraries and anyhow for top-level binaries.".into(),
                concurrency_and_resources: "Send and Sync marker traits guarantee data-race freedom at compile time. Use tokio/async-channel for async pipelines. Use parking_lot or tokio::sync synchronization primitives.".into(),
                security_considerations: vec![
                    "Audit all unsafe code blocks; require // SAFETY: invariants comment".into(),
                    "Use cargo-audit to check for vulnerabilities in crate dependency trees".into(),
                    "Defend against denial-of-service via bounded allocations and input validation".into(),
                ],
                linters_and_formatters: vec!["cargo clippy -- -D warnings".into(), "cargo fmt --check".into()],
                test_frameworks_and_checks: vec![
                    "cargo test".into(),
                    "cargo check".into(),
                    "proptest / quickcheck for property tests".into(),
                ],
                code_review_checklist: vec![
                    "Are all error cases propagated or logged rather than dropped/unwrapped?".into(),
                    "Are async tasks cancellation-safe and freed on shutdown?".into(),
                    "Is unsafe code zero or rigorously justified with invariant comments?".into(),
                    "Does cargo clippy pass with zero warnings?".into(),
                ],
                required_tools: vec!["cargo".into(), "rustc".into()],
            },
        );

        // 2. TypeScript Profile
        profiles.insert(
            SupportedLanguage::TypeScript,
            LanguageProfile {
                language: SupportedLanguage::TypeScript,
                name: "TypeScript".into(),
                primary_extensions: vec!["ts".into(), "tsx".into(), "mts".into(), "cts".into()],
                manifest_files: vec!["tsconfig.json".into(), "package.json".into()],
                baseline_version: "TypeScript 5.0+ (strict mode enabled)".into(),
                official_docs: vec![
                    "https://www.typescriptlang.org/docs/handbook/".into(),
                    "https://www.typescriptlang.org/tsconfig".into(),
                ],
                google_style_guide: Some("https://google.github.io/styleguide/tsguide.html".into()),
                sota_skill_id: "sota-javascript-typescript".into(),
                idiomatic_patterns: vec![
                    "Enable strict: true in tsconfig.json; noImplicitAny, strictNullChecks".into(),
                    "Discriminated unions with exhaustive switch narrowing (never type)".into(),
                    "Runtime schema validation for boundary data using Zod or Valibot".into(),
                    "Readonly<T> and as const assertion for immutable domain configurations".into(),
                ],
                common_antipatterns: vec![
                    "Indiscriminate use of 'any' type escaping type checking guarantees".into(),
                    "Non-null assertion operator (!) bypassing compile-time safety checks".into(),
                    "Treating compile-time interfaces as runtime validations for external JSON".into(),
                    "Floating unhandled Promises and missing catch handlers".into(),
                ],
                type_and_error_handling: "Structural type system. Distinguish compile-time types from runtime reality. Throw Error instances, never plain strings. Prefer Result-like tuples or error unions for predictable flows.".into(),
                concurrency_and_resources: "Single-threaded event loop. Non-blocking asynchronous I/O via async/await and Promises. Handle AbortController signals for fetch/stream cancellation.".into(),
                security_considerations: vec![
                    "Strict prototype pollution defenses when merging untrusted objects".into(),
                    "Validate and sanitize all external query/body inputs before processing".into(),
                    "Run npm audit or pnpm audit for CVE scans in node_modules".into(),
                ],
                linters_and_formatters: vec!["eslint --max-warnings 0".into(), "prettier --check".into()],
                test_frameworks_and_checks: vec![
                    "tsc --noEmit".into(),
                    "vitest / jest".into(),
                    "npm test".into(),
                ],
                code_review_checklist: vec![
                    "Are any 'any' types introduced? If so, replace with 'unknown' + narrowing".into(),
                    "Are async functions properly awaited with error boundaries?".into(),
                    "Does tsc --noEmit pass with zero type errors?".into(),
                ],
                required_tools: vec!["tsc".into(), "node".into(), "npm".into()],
            },
        );

        // 3. JavaScript Profile
        profiles.insert(
            SupportedLanguage::JavaScript,
            LanguageProfile {
                language: SupportedLanguage::JavaScript,
                name: "JavaScript".into(),
                primary_extensions: vec!["js".into(), "jsx".into(), "mjs".into(), "cjs".into()],
                manifest_files: vec!["package.json".into()],
                baseline_version: "ECMAScript 2022+ / Node 18+ LTS".into(),
                official_docs: vec![
                    "https://developer.mozilla.org/en-US/docs/Web/JavaScript".into(),
                    "https://nodejs.org/docs/latest/api/".into(),
                ],
                google_style_guide: Some("https://google.github.io/styleguide/jsguide.html".into()),
                sota_skill_id: "sota-javascript-typescript".into(),
                idiomatic_patterns: vec![
                    "ES Modules (import/export) with strict mode enabled".into(),
                    "Modern async/await over raw callback pyramids".into(),
                    "Destructuring with default values and optional chaining (?.) / nullish coalescing (??)".into(),
                    "Structured cloning (structuredClone) over JSON.parse(JSON.stringify())".into(),
                ],
                common_antipatterns: vec![
                    "Loose equality (==) instead of strict identity (===)".into(),
                    "Modifying Object.prototype or global object state".into(),
                    "Uncaught rejected promises crashing Node.js runtime process".into(),
                    "Synchronous blocking file operations (fs.readFileSync) in web server request paths".into(),
                ],
                type_and_error_handling: "Dynamic typing. Validate boundaries with JSDoc @type annotations or schema validators. Reject promises with Error instances.".into(),
                concurrency_and_resources: "Event loop with microtask/macrotask queues. Avoid blocking the event loop with heavy CPU computations. Close stream handles explicitly.".into(),
                security_considerations: vec![
                    "Prevent Cross-Site Scripting (XSS) via safe DOM rendering or template escaping".into(),
                    "Avoid eval() and Function constructor with user inputs (RCE risk)".into(),
                    "Validate redirect targets and URL protocols against javascript: schemes".into(),
                ],
                linters_and_formatters: vec!["eslint .".into(), "prettier --check .".into()],
                test_frameworks_and_checks: vec!["npm test".into(), "jest".into(), "vitest".into()],
                code_review_checklist: vec![
                    "Are all promise rejections handled?".into(),
                    "Are inputs validated before consumption in sensitive APIs?".into(),
                    "Is strict equality (===) used throughout?".into(),
                ],
                required_tools: vec!["node".into(), "npm".into()],
            },
        );

        // 4. Python Profile
        profiles.insert(
            SupportedLanguage::Python,
            LanguageProfile {
                language: SupportedLanguage::Python,
                name: "Python".into(),
                primary_extensions: vec!["py".into(), "pyi".into()],
                manifest_files: vec![
                    "pyproject.toml".into(),
                    "requirements.txt".into(),
                    "setup.py".into(),
                    "Pipfile".into(),
                ],
                baseline_version: "Python 3.10 - 3.13 (PEP 585 / PEP 604 type hints)".into(),
                official_docs: vec![
                    "https://docs.python.org/3/".into(),
                    "https://peps.python.org/pep-0008/".into(),
                    "https://typing.readthedocs.io/".into(),
                ],
                google_style_guide: Some("https://google.github.io/styleguide/pyguide.html".into()),
                sota_skill_id: "sota-python".into(),
                idiomatic_patterns: vec![
                    "Type annotations on all public functions and dataclasses/Pydantic models".into(),
                    "Context managers (with / async with) for deterministic file and lock cleanup".into(),
                    "Comprehensions and generator expressions for memory-efficient iteration".into(),
                    "Pathlib over raw os.path string manipulations".into(),
                ],
                common_antipatterns: vec![
                    "Mutable default arguments (def foo(x=[])) causing shared state pollution".into(),
                    "Bare 'except:' catching SystemExit and KeyboardInterrupt unexpectedly".into(),
                    "Using eval(), exec(), or pickle.loads() on untrusted external data".into(),
                    "Mixing blocking sync calls inside asyncio event loops".into(),
                ],
                type_and_error_handling: "Gradual typing via Mypy/Pyright. Raise specific Exception subclasses. Use try/except/else/finally for clear separation of success and cleanup.".into(),
                concurrency_and_resources: "Global Interpreter Lock (GIL) considerations. Use asyncio for I/O concurrency; multiprocessing for CPU-bound tasks. Always close database connection pools.".into(),
                security_considerations: vec![
                    "Sanitize parameters to prevent SQL injection (use parameterized ORM queries)".into(),
                    "Never deserialize untrusted payloads with pickle, yaml.load(), or marshal".into(),
                    "Use pip-audit or safety to detect vulnerable dependencies".into(),
                ],
                linters_and_formatters: vec!["ruff check .".into(), "black --check .".into(), "mypy .".into()],
                test_frameworks_and_checks: vec!["pytest -v".into(), "python -m unittest".into()],
                code_review_checklist: vec![
                    "Are type hints complete and validated with mypy?".into(),
                    "Are resources cleaned up via with-statements?".into(),
                    "Are exceptions specific rather than bare except:?".into(),
                ],
                required_tools: vec!["python".into(), "pytest".into()],
            },
        );

        // 5. C Profile
        profiles.insert(
            SupportedLanguage::C,
            LanguageProfile {
                language: SupportedLanguage::C,
                name: "C".into(),
                primary_extensions: vec!["c".into(), "h".into()],
                manifest_files: vec!["Makefile".into(), "CMakeLists.txt".into()],
                baseline_version: "C11 / C17 (ISO/IEC 9899)".into(),
                official_docs: vec![
                    "https://en.cppreference.com/w/c".into(),
                    "https://wiki.sei.cmu.edu/confluence/display/c/SEI+CERT+C+Coding+Standard".into(),
                ],
                google_style_guide: None,
                sota_skill_id: "sota-c-cpp".into(),
                idiomatic_patterns: vec![
                    "Check all return codes and pointer arguments for NULL before dereferencing".into(),
                    "Explicit buffer capacity tracking; use snprintf() instead of sprintf()/strcpy()".into(),
                    "Deterministic paired resource allocation (init/cleanup or malloc/free in same layer)".into(),
                    "Static internal linkage for file-scoped helper functions".into(),
                ],
                common_antipatterns: vec![
                    "Buffer overflow / off-by-one errors writing past array bounds".into(),
                    "Use-after-free and double-free memory bugs".into(),
                    "Uninitialized variable reads causing undefined behavior".into(),
                    "Integer overflow leading to wraparound allocation exploits".into(),
                ],
                type_and_error_handling: "Static weakly-typed system. Return integer status codes or error enums. Set errno appropriately. Output pointers must be cleared on error.".into(),
                concurrency_and_resources: "Manual resource tracking. Use C11 stdatomic.h or POSIX pthreads. Guard against race conditions and lock ordering deadlocks.".into(),
                security_considerations: vec![
                    "Compile with -Wall -Wextra -Werror -Wpedantic and -D_FORTIFY_SOURCE=2".into(),
                    "Run AddressSanitizer (ASan) and UndefinedBehaviorSanitizer (UBSan)".into(),
                    "Avoid dangerous functions (gets, strcpy, sprintf, strcat)".into(),
                ],
                linters_and_formatters: vec!["clang-tidy".into(), "clang-format --dry-run".into(), "cppcheck".into()],
                test_frameworks_and_checks: vec!["ctest".into(), "make test".into()],
                code_review_checklist: vec![
                    "Are all memory allocations matched with free() in every exit branch?".into(),
                    "Are array bounds and buffer lengths strictly checked?".into(),
                    "Does the build compile cleanly without warnings?".into(),
                ],
                required_tools: vec!["gcc".into(), "clang".into(), "cmake".into()],
            },
        );

        // 6. C++ Profile
        profiles.insert(
            SupportedLanguage::Cpp,
            LanguageProfile {
                language: SupportedLanguage::Cpp,
                name: "C++".into(),
                primary_extensions: vec!["cpp".into(), "hpp".into(), "cc".into(), "cxx".into(), "h".into()],
                manifest_files: vec!["CMakeLists.txt".into(), "Makefile".into()],
                baseline_version: "C++17 / C++20 (ISO/IEC 14882)".into(),
                official_docs: vec![
                    "https://en.cppreference.com/w/cpp".into(),
                    "https://isocpp.github.io/CppCoreGuidelines/CppCoreGuidelines".into(),
                ],
                google_style_guide: Some("https://google.github.io/styleguide/cppguide.html".into()),
                sota_skill_id: "sota-c-cpp".into(),
                idiomatic_patterns: vec![
                    "Resource Acquisition Is Initialization (RAII) for all resource management".into(),
                    "Smart pointers (std::unique_ptr, std::shared_ptr) instead of raw owning pointers".into(),
                    "Move semantics (std::move, rvalue references) avoiding unnecessary deep copies".into(),
                    "Standard library algorithms and ranges (std::ranges) over raw indexing loops".into(),
                    "Const-correctness: mark methods and parameters const wherever possible".into(),
                ],
                common_antipatterns: vec![
                    "Manual delete calls and owning raw pointers violating RAII".into(),
                    "Dangling references to temporaries or moved-from objects".into(),
                    "Catch-all catch(...) swallowing unexpected exceptions silently".into(),
                    "Undefined behavior via signed integer overflow or out-of-bounds indexing".into(),
                ],
                type_and_error_handling: "Static strongly typed with templates and concepts (C++20). Prefer exceptions for exceptional failures; std::optional or std::expected (C++23) for expected failures.".into(),
                concurrency_and_resources: "std::jthread (C++20) for cooperative cancellation. std::unique_lock and std::scoped_lock to avoid deadlocks. std::atomic for lockless variables.".into(),
                security_considerations: vec![
                    "Use AddressSanitizer, LeakSanitizer, and ThreadSanitizer during testing".into(),
                    "Enable compiler security flags (-fstack-protector-strong, ASLR, DEP)".into(),
                    "Avoid raw pointer arithmetic across buffer boundaries".into(),
                ],
                linters_and_formatters: vec!["clang-tidy -p .".into(), "clang-format --dry-run".into()],
                test_frameworks_and_checks: vec!["ctest --output-on-failure".into(), "cmake --build .".into()],
                code_review_checklist: vec![
                    "Are raw owning pointers completely avoided?".into(),
                    "Is RAII followed for all file, memory, and lock handles?".into(),
                    "Are move operations noexcept where expected?".into(),
                ],
                required_tools: vec!["cmake".into(), "clang++".into(), "g++".into()],
            },
        );

        // 7. C# Profile (.NET)
        profiles.insert(
            SupportedLanguage::CSharp,
            LanguageProfile {
                language: SupportedLanguage::CSharp,
                name: "C#".into(),
                primary_extensions: vec!["cs".into(), "csx".into()],
                manifest_files: vec![".csproj".into(), ".sln".into()],
                baseline_version: "C# 12 / .NET 8 LTS (Nullable Reference Types enabled)".into(),
                official_docs: vec![
                    "https://learn.microsoft.com/en-us/dotnet/csharp/".into(),
                    "https://learn.microsoft.com/en-us/dotnet/core/".into(),
                ],
                google_style_guide: Some("https://google.github.io/styleguide/csharp-style.html".into()),
                sota_skill_id: "sota-dotnet".into(),
                idiomatic_patterns: vec![
                    "Enable <Nullable>enable</Nullable> in all project manifests".into(),
                    "using statements or await using declarations for IDisposable/IAsyncDisposable".into(),
                    "Async/await with CancellationToken passed through entire call hierarchy".into(),
                    "Record types and init-only properties for immutable domain models".into(),
                    "Dependency injection and options pattern via Microsoft.Extensions.DependencyInjection".into(),
                ],
                common_antipatterns: vec![
                    "async void methods (except event handlers) leading to uncatchable crashes".into(),
                    ".Result or .Wait() blocking calls causing threadpool starvation and deadlocks".into(),
                    "Discarding CancellationTokens or ignoring OperationCanceledException".into(),
                    "Ignoring nullability warnings with unchecked ! null-forgiving operators".into(),
                ],
                type_and_error_handling: "Static object-oriented and functional type system. Explicit nullable reference types. Throw descriptive System.Exception subclasses; never swallow exceptions in empty catch blocks.".into(),
                concurrency_and_resources: "Task Parallel Library (TPL). SemaphoreSlim for async synchronization. Always respect CancellationToken. ConfigureAwait(false) in non-UI libraries.".into(),
                security_considerations: vec![
                    "Parameterized queries with Entity Framework Core or Dapper (prevent SQL injection)".into(),
                    "Disable dangerous binary formatters; use System.Text.Json".into(),
                    "dotnet list package --vulnerable for NuGet vulnerability auditing".into(),
                ],
                linters_and_formatters: vec!["dotnet format --verify-no-changes".into(), "Roslyn analyzers".into()],
                test_frameworks_and_checks: vec!["dotnet test".into(), "dotnet build /warnaserror".into()],
                code_review_checklist: vec![
                    "Is async void avoided in non-event handlers?".into(),
                    "Are IDisposable instances disposed via using blocks?".into(),
                    "Are cancellation tokens forwarded through async calls?".into(),
                ],
                required_tools: vec!["dotnet".into()],
            },
        );

        // 8. Java Profile (JVM)
        profiles.insert(
            SupportedLanguage::Java,
            LanguageProfile {
                language: SupportedLanguage::Java,
                name: "Java".into(),
                primary_extensions: vec!["java".into()],
                manifest_files: vec!["pom.xml".into(), "build.gradle".into(), "build.gradle.kts".into()],
                baseline_version: "Java 17 / 21 LTS (Modern OpenJDK)".into(),
                official_docs: vec![
                    "https://docs.oracle.com/en/java/javase/21/docs/api/".into(),
                    "https://dev.java/learn/".into(),
                ],
                google_style_guide: Some("https://google.github.io/styleguide/javaguide.html".into()),
                sota_skill_id: "sota-jvm".into(),
                idiomatic_patterns: vec![
                    "Records for immutable data transfer objects (Java 16+)".into(),
                    "Pattern matching for instanceof and switch expressions (Java 21)".into(),
                    "Try-with-resources statement for deterministic AutoCloseable resource management".into(),
                    "Streams API and Optional<T> for clean functional pipelines (avoid nulls)".into(),
                ],
                common_antipatterns: vec![
                    "Returning null from methods returning collections (return Collections.emptyList())".into(),
                    "Catching Throwable or Exception without logging or rethrowing".into(),
                    "Calling Thread.stop() or ignoring Thread.currentThread().interrupt()".into(),
                    "Double-checked locking without volatile keyword".into(),
                ],
                type_and_error_handling: "Static class-based type system with generics. Checked vs unchecked exceptions. Create custom RuntimeException subclasses for clean service architecture.".into(),
                concurrency_and_resources: "java.util.concurrent abstractions (ExecutorService, CompletableFuture). Virtual Threads (Java 21 Project Loom) for high-throughput I/O.".into(),
                security_considerations: vec![
                    "Prevent XML External Entity (XXE) attacks in XML parsers".into(),
                    "Never use default Java object serialization with untrusted streams".into(),
                    "Dependency vulnerability scanning via OWASP Dependency-Check or Maven Snyk".into(),
                ],
                linters_and_formatters: vec!["mvn spotbugs:check".into(), "mvn checkstyle:check".into()],
                test_frameworks_and_checks: vec!["mvn test".into(), "gradle test".into()],
                code_review_checklist: vec![
                    "Are AutoCloseable resources wrapped in try-with-resources?".into(),
                    "Are InterruptedExceptions handled properly by resetting interrupt flag?".into(),
                    "Does the code avoid raw types in generics?".into(),
                ],
                required_tools: vec!["java".into(), "javac".into(), "mvn".into()],
            },
        );

        // 9. Go Profile
        profiles.insert(
            SupportedLanguage::Go,
            LanguageProfile {
                language: SupportedLanguage::Go,
                name: "Go".into(),
                primary_extensions: vec!["go".into()],
                manifest_files: vec!["go.mod".into(), "go.sum".into()],
                baseline_version: "Go 1.21 - 1.23".into(),
                official_docs: vec![
                    "https://go.dev/doc/effective_go".into(),
                    "https://go.dev/ref/spec".into(),
                    "https://pkg.go.dev/std".into(),
                ],
                google_style_guide: Some("https://google.github.io/styleguide/go/".into()),
                sota_skill_id: "sota-golang".into(),
                idiomatic_patterns: vec![
                    "Explicit error handling: if err != nil { return fmt.Errorf(\"...: %w\", err) }".into(),
                    "Accept interfaces, return structs".into(),
                    "context.Context as first argument for deadline propagation and cancellation".into(),
                    "defer statement for guaranteed file and mutex unlocking".into(),
                    "Channels for orchestration and synchronization; sync.Mutex for simple state protection".into(),
                ],
                common_antipatterns: vec![
                    "Ignoring returned errors (_ = fn()) in production logic".into(),
                    "Goroutine leaks: launching goroutines without stop signal or context cancellation".into(),
                    "Data races on shared maps or slices without synchronization".into(),
                    "Over-abstracting with unnecessary empty interfaces (interface{})".into(),
                ],
                type_and_error_handling: "Static structural typing with implicit interface satisfaction. No exceptions; errors are values. Use errors.Is and errors.As for wrapped errors.".into(),
                concurrency_and_resources: "Goroutines and CSP channels. Run tests with -race flag. Always propagate context.Context. Ensure deferred cleanup handles run on all exit paths.".into(),
                security_considerations: vec![
                    "Use parameterized sql.DB queries to prevent SQL injection".into(),
                    "govulncheck ./... to scan for known Go security vulnerabilities".into(),
                    "Defend against path traversal via filepath.Clean and boundary checks".into(),
                ],
                linters_and_formatters: vec!["gofmt -d .".into(), "go vet ./...".into(), "golangci-lint run".into()],
                test_frameworks_and_checks: vec!["go test -v -race ./...".into(), "go test -cover ./...".into()],
                code_review_checklist: vec![
                    "Are all returned errors checked and wrapped with context?".into(),
                    "Are goroutines guaranteed to terminate on context cancellation?".into(),
                    "Does go test -race pass with zero data races?".into(),
                ],
                required_tools: vec!["go".into()],
            },
        );

        // 10. SQL Profile
        profiles.insert(
            SupportedLanguage::Sql,
            LanguageProfile {
                language: SupportedLanguage::Sql,
                name: "SQL".into(),
                primary_extensions: vec!["sql".into()],
                manifest_files: vec!["schema.sql".into(), "migrations".into(), ".sqlfluff".into()],
                baseline_version: "ANSI SQL:2016 / PostgreSQL 15+ / SQLite 3".into(),
                official_docs: vec![
                    "https://www.postgresql.org/docs/current/".into(),
                    "https://www.sqlite.org/docs.html".into(),
                ],
                google_style_guide: None,
                sota_skill_id: "sota-databases".into(),
                idiomatic_patterns: vec![
                    "Strict transactional DDL (BEGIN ... COMMIT) for all schema migrations".into(),
                    "Parameterized queries / prepared statements everywhere (no raw string interpolation)".into(),
                    "Explicit foreign key constraints and ON DELETE CASCADE/RESTRICT policies".into(),
                    "Appropriate B-tree or partial indexes for high-cardinality WHERE and JOIN conditions".into(),
                    "Reversible migrations (paired UP and DOWN scripts)".into(),
                ],
                common_antipatterns: vec![
                    "Dynamic string concatenation constructing SQL queries (SQL Injection risk)".into(),
                    "Running table ALTER or DROP in production without downtime or lock analysis".into(),
                    "SELECT * in production queries triggering full table scans and unexpected network payload".into(),
                    "Missing transactions around multi-step balance or inventory updates".into(),
                ],
                type_and_error_handling: "Relational schema type system. Enforce NOT NULL and CHECK constraints at database level. Handle transaction serialization failures (e.g. 40001 retry loop).".into(),
                concurrency_and_resources: "ACID transactions. Understand isolation levels (Read Committed, Repeatable Read, Serializable). Avoid long-lived locks blocking concurrent writers.".into(),
                security_considerations: vec![
                    "Always use parameterized statements ($1, $2 or :name) to prevent SQL injection".into(),
                    "Principle of least privilege: application role must not own superuser permissions".into(),
                    "Never run migration scripts against production without verified dry-run".into(),
                ],
                linters_and_formatters: vec!["sqlfluff lint --dialect postgres".into(), "sqlfluff format".into()],
                test_frameworks_and_checks: vec![
                    "pg_prove / test database migrations".into(),
                    "sqlite3 in-memory isolated schema test".into(),
                ],
                code_review_checklist: vec![
                    "Are queries parameterized with zero string concatenation?".into(),
                    "Are schema changes backwards-compatible and wrapped in transactions?".into(),
                    "Are indexes created concurrently where table locks are a risk?".into(),
                ],
                required_tools: vec!["sqlfluff".into(), "psql".into(), "sqlite3".into()],
            },
        );

        Self { profiles }
    }

    pub fn get(&self, lang: SupportedLanguage) -> Option<&LanguageProfile> {
        self.profiles.get(&lang)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&LanguageProfile> {
        SupportedLanguage::from_str_loose(name).and_then(|l| self.get(l))
    }

    pub fn all_profiles(&self) -> Vec<&LanguageProfile> {
        self.profiles.values().collect()
    }
}

/// Detects languages and project architectures across a repository filesystem.
pub struct RepositoryLanguageDetector;

impl RepositoryLanguageDetector {
    /// Inspects the target repository root and discovers all active programming languages.
    pub fn detect(workspace_root: impl AsRef<Path>) -> MultiLanguageDetectionResult {
        let root = workspace_root.as_ref();
        let registry = LanguageProfileRegistry::new();

        let mut counts: HashMap<SupportedLanguage, usize> = HashMap::new();
        let mut manifests: HashMap<SupportedLanguage, Vec<String>> = HashMap::new();
        let mut sample_files: HashMap<SupportedLanguage, Vec<String>> = HashMap::new();
        let mut total_code_files = 0;

        // 1. Check manifests first
        for profile in registry.all_profiles() {
            for manifest_name in &profile.manifest_files {
                let p = root.join(manifest_name);
                if p.exists() {
                    manifests
                        .entry(profile.language)
                        .or_default()
                        .push(manifest_name.clone());
                }
            }
        }

        // 2. Walk directory to inspect file extensions (bounded to max depth 8, skipping .git/target/node_modules)
        let walker = walkdir::WalkDir::new(root)
            .follow_links(false)
            .max_depth(8)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                name != ".git"
                    && name != "target"
                    && name != "node_modules"
                    && name != ".idea"
                    && name != "dist"
                    && name != "build"
                    && name != "__pycache__"
                    && name != "venv"
                    && name != ".venv"
            });

        for entry in walker.filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let path = entry.path();
                let ext = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let rel_path = path.strip_prefix(root).unwrap_or(path).to_string_lossy().to_string();

                for profile in registry.all_profiles() {
                    if profile.primary_extensions.iter().any(|e| e == &ext) {
                        *counts.entry(profile.language).or_insert(0) += 1;
                        total_code_files += 1;

                        let samples = sample_files.entry(profile.language).or_default();
                        if samples.len() < 5 {
                            samples.push(rel_path.clone());
                        }
                    }
                }
            }
        }

        // 3. Score confidence
        let mut detected_languages = Vec::new();

        for lang in SupportedLanguage::all() {
            let file_count = counts.get(lang).copied().unwrap_or(0);
            let matched_mans = manifests.get(lang).cloned().unwrap_or_default();
            let samples = sample_files.get(lang).cloned().unwrap_or_default();

            if file_count > 0 || !matched_mans.is_empty() {
                let mut score: f32 = 0.0;

                // Having a build manifest is strong evidence
                if !matched_mans.is_empty() {
                    score += 0.5;
                }

                // File count contribution
                if file_count >= 10 {
                    score += 0.5;
                } else if file_count >= 1 {
                    score += 0.3;
                }

                score = score.min(1.0);

                detected_languages.push(LanguageEvidence {
                    language: *lang,
                    file_count,
                    matched_manifests: matched_mans,
                    matched_configs: Vec::new(),
                    sample_files: samples,
                    confidence_score: score,
                });
            }
        }

        // Sort by file count & confidence descending
        detected_languages.sort_by(|a, b| {
            b.file_count
                .cmp(&a.file_count)
                .then(b.confidence_score.partial_cmp(&a.confidence_score).unwrap_or(std::cmp::Ordering::Equal))
        });

        let primary_language = detected_languages.first().map(|d| d.language);
        let is_polyglot = detected_languages.len() > 1;

        MultiLanguageDetectionResult {
            workspace_root: root.to_path_buf(),
            total_code_files,
            detected_languages,
            primary_language,
            is_polyglot,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_10_language_profiles_registered() {
        let registry = LanguageProfileRegistry::new();
        assert_eq!(registry.all_profiles().len(), 10);

        for lang in SupportedLanguage::all() {
            let profile = registry.get(*lang).expect("Profile must exist");
            assert!(!profile.name.is_empty());
            assert!(!profile.official_docs.is_empty());
            assert!(!profile.idiomatic_patterns.is_empty());
            assert!(!profile.common_antipatterns.is_empty());
            assert!(!profile.security_considerations.is_empty());
            assert!(!profile.linters_and_formatters.is_empty());
            assert!(!profile.test_frameworks_and_checks.is_empty());
            assert!(!profile.code_review_checklist.is_empty());
            assert!(!profile.required_tools.is_empty());
        }
    }

    #[test]
    fn test_language_detection_on_current_repo() {
        let result = RepositoryLanguageDetector::detect(".");
        assert!(result.total_code_files > 0);
        assert!(result.has_language(SupportedLanguage::Rust));

        let rust_evidence = result
            .detected_languages
            .iter()
            .find(|d| d.language == SupportedLanguage::Rust)
            .unwrap();
        assert!(rust_evidence.file_count > 10);
        assert!(rust_evidence.matched_manifests.contains(&"Cargo.toml".to_string()));
        assert!(rust_evidence.confidence_score >= 0.8);
    }

    #[test]
    fn test_language_from_str_loose() {
        assert_eq!(SupportedLanguage::from_str_loose("rust"), Some(SupportedLanguage::Rust));
        assert_eq!(SupportedLanguage::from_str_loose("TypeScript"), Some(SupportedLanguage::TypeScript));
        assert_eq!(SupportedLanguage::from_str_loose("py"), Some(SupportedLanguage::Python));
        assert_eq!(SupportedLanguage::from_str_loose("C++"), Some(SupportedLanguage::Cpp));
        assert_eq!(SupportedLanguage::from_str_loose("c#"), Some(SupportedLanguage::CSharp));
        assert_eq!(SupportedLanguage::from_str_loose("golang"), Some(SupportedLanguage::Go));
        assert_eq!(SupportedLanguage::from_str_loose("postgresql"), Some(SupportedLanguage::Sql));
    }
}
