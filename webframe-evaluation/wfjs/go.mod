module wfjs

go 1.27.1

require tractor.dev/webframe v0.0.0

require (
	github.com/crgimenes/glaze v0.0.0-00010101000000-000000000000 // indirect
	github.com/ebitengine/purego v0.11.1 // indirect
)

// A checkout of github.com/tractordev/webframe with ../webframe.diff applied.
replace tractor.dev/webframe => ../webframe-src

replace github.com/crgimenes/glaze => github.com/progrium/glaze v0.0.0-20261024141632-aa23a9a95cce
