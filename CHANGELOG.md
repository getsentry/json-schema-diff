# Changelog and versioning
## 0.1.8

### New Features ✨

- (release) Replace release bot with GH app by @Jeffreyhung in [#41](https://github.com/getsentry/json-schema-diff/pull/41)
- Add enum support with context-aware breaking change detection by @domdomegg in [#46](https://github.com/getsentry/json-schema-diff/pull/46)
- Add string validation support (pattern, minLength, maxLength) by @domdomegg in [#51](https://github.com/getsentry/json-schema-diff/pull/51)
- Support format by @brhutchins in [#45](https://github.com/getsentry/json-schema-diff/pull/45)
- Support custom ids and $defs from draft 2020-12 by @cakemanny in [#44](https://github.com/getsentry/json-schema-diff/pull/44)

### Internal Changes 🔧

#### Release

- Fix changelog-preview permissions by @BYK in [#54](https://github.com/getsentry/json-schema-diff/pull/54)
- Switch from action-prepare-release to Craft by @BYK in [#52](https://github.com/getsentry/json-schema-diff/pull/52)

#### Other

- Use pull_request_target for changelog preview by @BYK in [#53](https://github.com/getsentry/json-schema-diff/pull/53)

### Other

- Fix clippy lints by @untitaker in [#42](https://github.com/getsentry/json-schema-diff/pull/42)

## 0.1.7

### Various fixes & improvements

- fix: Add CLI install instructions (#37) by @untitaker
- support `exclusive` keywords (#35) by @6293
- do not clone Schema, clone only when the inner struct is Bool (#34) by @6293
- rm unnecessary boxing (#33) by @6293
- compare anyOf based on handmade diff score (#32) by @6293

## 0.1.6

### Various fixes & improvements

- support const keyword (#27) by @6293
- include input values in snapshot for a smooth review (#28) by @6293
- chore: consistent return type among methods (#29) by @6293
- split testsuite into files (#24) by @6293
- split multiple types into anyOf subschema (#20) by @6293
- feat: Add support for changes to "required" properties (#19) by @lynnagara
- doc: Remove feature list from README (#18) by @untitaker

## 0.1.5

### Various fixes & improvements

- support for minimum/maximum changes (#13) by @6293
- doc: Make the purpose more clear in README (#12) by @untitaker
- fix: Reenable help output for json-schema-diff (#11) by @untitaker

## 0.1.4

### Various fixes & improvements

- ref: Rewrite the crate and add support for references (#10) by @untitaker

## 0.1.3

### Various fixes & improvements

- fix: anyOf is not order-sensitive (#9) by @untitaker
- fix: Fix bug where additionalProperties was not true in changeset (#7) by @untitaker
- fix: Add another failing test and fix licensing metadata (#8) by @untitaker
- fix: Implement rudimentary support for anyOf and array items (#6) by @untitaker

## 0.1.2

### Various fixes & improvements

- fix: Add repository link (#4) by @untitaker
- fix: property removal is a breaking change if additionalProperties is false (#3) by @untitaker
- fix: Use a different replacement function in bump-version (#2) by @untitaker
- feat: Add codeowners (#1) by @untitaker

## 0.1.1

### Various fixes & improvements

- fix formatting (e2f76cf0) by @untitaker
- serialize is_breaking into output on CLI (249fee74) by @untitaker

## 0.1.0

### Various fixes & improvements

- fix license identifier (7e770213) by @untitaker
- fix cargo metadata (05593500) by @untitaker
- add bump-version script (9370fd8e) by @untitaker

