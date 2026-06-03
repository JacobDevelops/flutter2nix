# gradle2nix Standalone Usage

> **Status:** Placeholder — full guide added in Phase 5.

This document will explain how to use gradle2nix for non-Flutter Gradle projects
(Spring Boot, Kotlin JVM, plain Android), including:

- Installation via `nix run github:JacobDevelops/flutter2nix#gradle2nix`
- Running `gradle2nix lock` on any Gradle project
- Using `gradle2nix.lib.buildGradleProject` in your flake
- Handling large Maven BOM dependency graphs

See `crates/gradle2nix/` for implementation.
