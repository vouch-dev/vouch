# vouch

> A multi-ecosystem distributed package review system.

## Introduction

Software packages are usually used without review. Who's checked the code? Typically, no one but the author. Vouch is a review system designed to solve this problem.

Vouch evaluates software dependancies using user generted micro-reviews. Even single line reviews become powerful when aggregated!

## Getting Started

### Setup

First, lets setup Vouch. During setup we can optionally specify a git repository URL for publishing reviews.

`vouch setup https://github.com/<username>/reviews`

### Review

(Note: Vouch currently requires [VSCode](https://code.visualstudio.com/) to create reviews.)

Vouch supports multiple ecosystems and is extendable. For now, Python and Javascript support comes built-in. Lets review the [NPM](https://www.npmjs.com/) Javascript package [d3](https://www.npmjs.com/package/d3) at version `4.10.0`:

`vouch review d3 4.10.0`

### Peers

Subscribe to reviews created by other users using the command:

`vouch peer add https://github.com/vouch-dev/example-reviews`

### Sync

The sync command pulls new reviews from peers and publishes user generated reviews:

`vouch sync`

### Check

Reviews created using Vouch can be used to evaluate software project dependancies. Vouch extensions can discover ecosystem specific dependancy definition files. For example, the Python extension parses `Pipfile.lock` files.

The `check` command generates an evaluation report of local project dependancies based on available reviews:

`vouch check`
