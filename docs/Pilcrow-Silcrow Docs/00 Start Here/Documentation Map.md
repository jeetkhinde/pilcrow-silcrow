# Documentation Map

## Core Pilcrow

- [[Learning Path]]
- [[../01 Pilcrow/Mental Model]]
- [[../01 Pilcrow/Build Your First Page]]
- [[../01 Pilcrow/Routing]]
- [[../01 Pilcrow/Pages and Layouts]]
- [[../01 Pilcrow/Actions and Forms]]
- [[../01 Pilcrow/API Routes and Fragments]]
- [[../01 Pilcrow/Typed Routes and Params]]
- [[../01 Pilcrow/Config Env I18n Images and Head]]
- [[../Tutorials/Address Book/00 Introduction]] — end-to-end step-by-step tutorial: setup through FSR live fields

## Rendering and Interactivity

- [[../03 Rendering/Rendering Models]]
- [[../03 Rendering/Build an FSR Page]]
- [[../03 Rendering/Live Props and FSR]]
- [[../03 Rendering/FSR SSE Hub]]
- [[../03 Rendering/Islands]]
- [[../03 Rendering/React Islands]]
- [[../03 Rendering/Lists and Live Patching]]

## Silcrow

- [[../02 Silcrow/Silcrow Runtime]]
- [[../02 Silcrow/Directives and Bindings]]
- [[../02 Silcrow/Navigation and Live Connections]]
- [[../02 Silcrow/Atoms and Optimistic UI]]

## Runtime and Operations

- [[../04 Runtime/Middleware Hooks and Request Lifecycle]]
- [[../04 Runtime/Security and Limits]]
- [[../04 Runtime/Assets Dev Server and Deployment]]

## Reference

- [[Feature Status]]
- [[../05 Reference/FSR Ownership and Invalidation]] — what the app owns vs what the framework owns; invalidation patterns; anti-patterns
- [[../05 Reference/Removed and Legacy Features]]
- [[../05 Reference/Experimental Baked Pages]]
- [[../05 Reference/Documentation Workflow]]

## Maintenance Rule

Every public behavior should have four matching layers:

1. Implementation.
2. `pilcrow/registry.toml` feature contract.
3. MCP knowledge coverage when relevant.
4. Runnable test or executable example evidence.
