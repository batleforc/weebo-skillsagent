# Changelog
All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

- - -
## [v0.1.0](https://github.com/batleforc/weebo-skillsagent/compare/0c03f06cec0554d2bc9fd20c219b1e7e42ffd0d2..v0.1.0) - 2026-10-16
#### Features
- (**wska**) fail sync --check when a source is behind upstream - ([e5225e9](https://github.com/batleforc/weebo-skillsagent/commit/e5225e9004698607085ab521c64a95709c51649b)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) add doctor to check whether each skill can work - ([bbeb7d4](https://github.com/batleforc/weebo-skillsagent/commit/bbeb7d41bf77353cf06330e95ecae90a90a9fcd8)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) sync vendored sources with patch detection - ([be1f719](https://github.com/batleforc/weebo-skillsagent/commit/be1f71944afd740729414f1ebd1ea92c517da9b0)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) add status and list commands - ([bc79220](https://github.com/batleforc/weebo-skillsagent/commit/bc79220d27d71caf4563ad6b8407488c15302da3)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) apply install plans and wire the install command - ([7895800](https://github.com/batleforc/weebo-skillsagent/commit/789580054530ba1d29b464577aac8bc6274294e7)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) build and validate a declarative install plan - ([3c6e9ba](https://github.com/batleforc/weebo-skillsagent/commit/3c6e9baf4455722cc91f9c5650e9604a82d4b60e)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) classify target entries to protect foreign paths - ([808ab1a](https://github.com/batleforc/weebo-skillsagent/commit/808ab1ac4204b1323cb8462f4aaea80b221cabae)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) resolve install targets for project and user scope - ([9ee97d5](https://github.com/batleforc/weebo-skillsagent/commit/9ee97d5e025613b8610ca210d2d4be038aa4a4a9)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) resolve profiles with include composition - ([c2a04e5](https://github.com/batleforc/weebo-skillsagent/commit/c2a04e5027541fb9d7826cfbc2842d7fa0b17e15)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) scan the repo for skills, agents, mcp and installers - ([0306634](https://github.com/batleforc/weebo-skillsagent/commit/030663456dee0d800864c1ffe087e789ea50b8c0)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) parse profiles.toml and vendor.toml - ([3356ed7](https://github.com/batleforc/weebo-skillsagent/commit/3356ed7f918fccd25a3abba1479010f11b79b341)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) strengthen symlink test to prove non-following - ([c45d1d3](https://github.com/batleforc/weebo-skillsagent/commit/c45d1d3136afe456cc26cc0258195b09590e6b8e)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) hash vendored directories for drift detection - ([05ad987](https://github.com/batleforc/weebo-skillsagent/commit/05ad98761e75a31fc585c3005f481dddccc2c7f3)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) add workspace and CLI argument parsing - ([839f4c4](https://github.com/batleforc/weebo-skillsagent/commit/839f4c4c185a05e6d3722747e68a1ce1368372f6)) - Max Batleforc, Claude Opus 5 (1M context)
- judge CLI with rfc-ready and doc-review gates (agents B1) - ([d1c75d0](https://github.com/batleforc/weebo-skillsagent/commit/d1c75d00e2ace470182ac351d1e4b8a37af7983e)) - Max Batleforc, Claude Opus 5.5
- Judgement tipe - ([9fde5e6](https://github.com/batleforc/weebo-skillsagent/commit/9fde5e62f33732ae490966c73bea7e50bd57e0b7)) - Max Batleforc
- init work on subagents - ([768a77f](https://github.com/batleforc/weebo-skillsagent/commit/768a77f60790ddfe6beeab8be22094426f417d89)) - Max Batleforc
- Create Batlehub-RFC and Batlehub-DESIGN - ([91cc447](https://github.com/batleforc/weebo-skillsagent/commit/91cc447373fcfcc86252c502416bc3bf0b7bde4a)) - Max Batleforc
- new claud.md/agnets.md - ([bc6cb02](https://github.com/batleforc/weebo-skillsagent/commit/bc6cb02361a1c44dba98798406dbe2cd7a139df8)) - Max Batleforc
- add new skills - ([2b5d4a0](https://github.com/batleforc/weebo-skillsagent/commit/2b5d4a09fea57737e16c05153bb2de2f35888219)) - Max Batleforc
- add design, frontend and mcp profiles - ([103cefb](https://github.com/batleforc/weebo-skillsagent/commit/103cefb519af1561c2f2d6636908b026d5ba5a78)) - Max Batleforc, Claude Opus 5 (1M context)
- vendor five third-party skills - ([c44c766](https://github.com/batleforc/weebo-skillsagent/commit/c44c766a4028dbefe1c3be02d06d1b8e31cdd2d3)) - Max Batleforc, Claude Opus 5 (1M context)
- add profiles.toml and vendor.toml scaffolding - ([bf73763](https://github.com/batleforc/weebo-skillsagent/commit/bf73763487ddc05c0fb378478e6e7c19b4e06735)) - Max Batleforc, Claude Opus 5 (1M context)
#### Bug Fixes
- (**wska**) gate installers behind a flag and re-check ownership on unlink - ([dcf1798](https://github.com/batleforc/weebo-skillsagent/commit/dcf1798d102606f599fe22e43ea7759ad1f1eca4)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) don't advance commit on patch, copy-then-rename on fast-forward - ([1debce0](https://github.com/batleforc/weebo-skillsagent/commit/1debce044415b2c8df0990a659a8eed1a5facee0)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) make plan removal kind-aware and fix test coverage gap - ([c5ac451](https://github.com/batleforc/weebo-skillsagent/commit/c5ac451357fa17354281846a27389f4d0a3cfa86)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) normalize both sides of the repo-root containment check - ([90174ec](https://github.com/batleforc/weebo-skillsagent/commit/90174ec6318bdfe4eabda5d80bce43d48ad8db1b)) - Max Batleforc, Claude Opus 5 (1M context)
- (**wska**) handle symlinks correctly and strengthen duplicate-name tests - ([a874a2f](https://github.com/batleforc/weebo-skillsagent/commit/a874a2f05fb1cd79da1a0eb6f65443e0a42b7e0c)) - Max Batleforc, Claude Opus 5 (1M context)
- stop the commit hooks reformatting vendored skills - ([cdefda9](https://github.com/batleforc/weebo-skillsagent/commit/cdefda9b0bed18d30844ad02f769f2d86d3a2335)) - Max Batleforc, Claude Opus 5 (1M context)
#### Documentation
- record the findings left unfixed on the wska spine - ([e6194e9](https://github.com/batleforc/weebo-skillsagent/commit/e6194e9b734ed0c2f6440828ef9441b4d72a1378)) - Max Batleforc, Claude Opus 5 (1M context)
- record the implementation amendments in the spec - ([d52f806](https://github.com/batleforc/weebo-skillsagent/commit/d52f8069f2ae9692a6e3047f92b5a66f0dd2ba7e)) - Max Batleforc, Claude Opus 5 (1M context)
- add skills spine design spec and implementation plan - ([3c4c7b6](https://github.com/batleforc/weebo-skillsagent/commit/3c4c7b6b6046efefdbd46792a92bcda588853ae9)) - Max Batleforc, Claude Opus 5 (1M context)
#### Build system
- wire wska into Taskfile and mise - ([57192d3](https://github.com/batleforc/weebo-skillsagent/commit/57192d366edab823dca54dc971d207910ede5337)) - Max Batleforc, Claude Opus 5 (1M context)
#### Miscellaneous Chores
- stop tracking .claude/skills, wska populates it - ([047f0a0](https://github.com/batleforc/weebo-skillsagent/commit/047f0a0a0d2b8c86fe971e15c3e316094b94e523)) - Max Batleforc, Claude Opus 5 (1M context)
#### Style
- format the workspace and clear the clippy warning - ([d3afe6d](https://github.com/batleforc/weebo-skillsagent/commit/d3afe6db27fa3e0aab99eab6833cf8f63d6d3ce6)) - Max Batleforc, Claude Opus 5 (1M context)

- - -

Changelog generated by [cocogitto](https://github.com/cocogitto/cocogitto).