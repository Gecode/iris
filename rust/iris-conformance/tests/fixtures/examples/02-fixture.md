# Extractor fixture

A larger ordinary fence contains an apparent example, which is not executable.

````markdown
```iris example
id: fixture-ignored
```
````

```iris example
id: fixture-literal
--- spec
x: word # literal \n
--- input no-final-newline
λ

--- params
{"n":"0007","label":"a,b"}
--- expected data
{"iris":"0.1","entries":[{"name":"x","type":"string","depth":0,"value":"λ"}]}
```

```iris example
id: fixture-binary
operation: parse
--- spec hex
23 ff 0d 0a 00
--- input hex
ff
--- params
{}
--- expected parse
{"iris":"0.1","parsed":true}
```

````iris example
id: fixture-hex-text
operation: dzn
--- spec hex
23 60 60 60 0a
--- input
--- expected dzn hex
78 20 3d 20 37 3b 0d 0a 00 ff
````

```iris example
id: fixture-catalog
operation: catalog
--- spec
instances { x: int }
--- input hex
31 0a
--- expected dzn no-final-newline
instances = 1;
```

```iris example
id: fixture-error
operation: parse
--- spec no-final-newline
@
--- expected error
{"code":"E-SPEC-CHARACTER","message":"unexpected character","input_location":null,"spec_location":{"origin":"spec.iris","line":1,"column":null},"context":null,"record_path":null}
```

```iris example
id: fixture-empty
--- spec no-final-newline

--- input no-final-newline
--- expected data
{"iris":"0.1","entries":[]}
```
