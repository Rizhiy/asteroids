# Guidelines for AI Agents Working on This Codebase

## Core Philosophy

**Simplicity First**: Always prefer the simplest solution that solves the actual problem. Complexity must justify itself - if something feels complicated, it probably is.

## Code Principles

### KISS (Keep It Simple, Stupid)

- **Most Important**: Prefer simple, straightforward solutions over complex ones
- Avoid over-engineering or premature optimization
- Write code that is easy to understand and maintain
- Question whether added complexity is truly necessary
- **When in doubt, choose the simpler approach**

### YAGNI (You Aren't Gonna Need It)

- Don't add functionality until it's actually needed
- Avoid speculative features or "just in case" code
- Focus on current requirements, not hypothetical future needs
- Remove unused code and features

### DRY (Don't Repeat Yourself)

- Do not duplicate existing functionality
- If similar functionality exists, make it more general and reuse it
- Extract common patterns into shared functions or modules
- Look for opportunities to consolidate repeated code
- **But: Simple duplication is better than complex abstraction**

## Development Methodology

### When Making Changes

- **Understand the actual requirement** - Don't assume or over-interpret
- **Start with the simplest solution** - Resist the urge to add abstractions
- **Listen to feedback** - If told something is getting too complex, stop and simplify.
- If a file was modified, and contains implementation different from how you left it, it was probably modified by other developers with a good reason. Adjust to it, rather than trying to revert to your own implementation.

### Commenting

- Comments should explain why something is happening, not what is happening.
- Prefer clear variable and function names over comments and long doc-strings.

### Testing Strategy

- Tests should run quickly
- **Don't create complex mocking infrastructure unless specifically requested**
- Test the actual implementation, not simulated versions

## Project specific rules

- Run `rustfmt` after you have made all of your changes.
