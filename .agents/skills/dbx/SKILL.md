```markdown
# dbx Development Patterns

> Auto-generated skill from repository analysis

## Overview
This skill teaches the core development patterns and conventions used in the `dbx` repository, a TypeScript codebase built with the Vue framework. You'll learn how to structure files, write and format code, adhere to commit message standards, and organize tests. This guide also provides suggested commands for common workflows to streamline your development process.

## Coding Conventions

### File Naming
- Use **camelCase** for all file names.
  - Example: `userProfile.vue`, `dataService.ts`

### Import Style
- Use **relative imports** for modules.
  - Example:
    ```typescript
    import userService from './userService'
    import utils from '../utils'
    ```

### Export Style
- Use **default exports** for modules.
  - Example:
    ```typescript
    export default {
      // module code
    }
    ```

### Commit Message Style
- Follow the **Conventional Commits** specification.
- Allowed prefixes: `fix`, `docs`, `build`
- Keep commit messages concise (average ~46 characters).
  - Example:
    ```
    fix: resolve data sync issue in userProfile
    docs: update README with setup instructions
    build: update dependencies for Vue 3
    ```

## Workflows

*No explicit workflows detected in the repository.*

## Testing Patterns

- **Test Framework:** Unknown (not detected)
- **Test File Pattern:** Name test files with the pattern `*.test.*`
  - Example: `userService.test.ts`
- Place test files alongside the modules they test or in a dedicated test directory.

  ```typescript
  // userService.test.ts
  import userService from './userService'

  describe('userService', () => {
    it('should fetch user data', () => {
      // test implementation
    })
  })
  ```

## Commands
| Command | Purpose |
|---------|---------|
| /commit-fix | Create a fix commit following conventions |
| /commit-docs | Create a docs commit following conventions |
| /commit-build | Create a build commit following conventions |
| /test | Run all test files matching *.test.* |
```