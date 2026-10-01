# Configuration Injection Contract

`configs/` is the single declarative configuration layer of the repository.
Product code must not scatter environment-specific constants across source files.

## Precedence

The intended merge order is:

`base -> profile -> local -> environment -> CLI`

Later layers override earlier layers. A runtime must expose the resolved configuration
as a typed object before entering domain logic.
