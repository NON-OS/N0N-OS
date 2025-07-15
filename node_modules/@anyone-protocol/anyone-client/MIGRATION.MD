# Migration guide: Transitioning to the new Ayone Protocol SDK structure

## **Overview**
This document provides a structured migration plan for transitioning both the **Python** and **NodeJS** SDKs to a **unified, object-oriented structure** that follows the **Stem** design principles.

## **Motivation**
### **Key points**
- **Improved maintainability:** By mirroring Stem's structure, the SDK remains modular and easy to maintain.
- **Consistency across SDKs:** Both Python and Node SDKs follow the same structure and design patterns, making it easier for developers to switch between them.
- **Enhanced readability & usability:** Object-oriented design ensures that data (e.g., circuit paths) is represented in a structured way rather than raw strings.
- **Better modularity:** Each class has a clear responsibility, making it easier to extend and modify the SDK.
- **Not reinventing a wheel:** Stem already has a comprehensive binary mangement and other battle-tested code we tried to implement from scratch. Example: `process.py` - for running Anon binary and listen to logs.
- **Tor developers native:** Stem is the official Tor Python library, and many Tor developers are already familiar with its structure and design principles.
- **Simpler SOCKS handling:** `socks.py` provides a clean, easy-to-use interface for SOCKS5 proxy handling, also it follows the patter on `controls.py` and `process.py`.
- **High-level scenarios:** `starters.py` provides high-level scenarios (e.g., VPN starter) that simplify complex operations.
- **Multi-step requests:** `templates.py` provides templates for multi-step requests, making it easier to interact with the Anon binary.
- **Consistent naming:** All SDKs will follow the same naming convention, making it easier to identify and use them.
- **Better documentation:** The new structure makes it easier to document and understand the SDK's functionality. Also documentation will be generated the same way as Stem's one.
- **Easier to test:** The new structure makes it easier to write unit tests for the SDK.
- **Anon changes compatibility:** Wrappers on top of Stem classes will make it easier to adapt to changes in the Anon binary.


## **Naming convention**
To ensure consistency, we will use the following naming for artifacts:
- **Python SDK:** `anyone-protocol-sdk`
- **NodeJS SDK:** `anyone-protocol-sdk`
Repository names will not include `anyone` but will have language prefix.
- **Python SDK:** `python-sdk`
- **NodeJS SDK:** `node-sdk`

## **Key migration principles**
1. **Structure and naming consistency**  
   - **Both SDKs should migrate to the same structure as Stem.**  
   - **Python SDK** will wrap **Stem classes** with the **same names**.  
   - **NodeJS SDK** will implement stem logic from scratch but **follow the same structure**.

2. **Object-oriented design**  
   - Instead of handling **raw strings**, SDK classes should use **typed objects** (e.g., Circuit path should be a list of fingerprints, not a string).  
   - Encapsulation should be applied to provide a **clean, intuitive API**.

3. **New files & module naming**  
   - **`socks.py` / `socks.ts`** → Implements **SOCKS5 proxy handling** (since Tor/Stem lacks built-in SOCKS support).  
   - **`process.py` / `process.ts`** → Handles **running and managing the Anon binary**.  
   - **`templates.py` / `templates.ts`** *→ Contains **multi-request templates** for operations like **fetching relay information for a circuit**.  
   - **`starters.py` / `starters.ts`** *→ Manages **end-to-end scenarios**, such as **VPN-starter** etc.
* `templates` and `starters` can be named differently (e.g., `operations.py` and `scenarios.py`).
* `starters` can be even extracted later to a separate package (e.g., `anyone-protocol-starters`).
---

## **New SDK structure**
### **Python SDK (`python-sdk/`)**
```
python-sdk/
│── anyone_protocol_sdk/
│   ├── control.py     # Control port client implementation
│   ├── process.py     # Handles Anon binary execution
│   ├── socks.py       # SOCKS5 client implementation
│   ├── proxy.py       # Proxy management
│   ├── templates.py   # Multi-step request templates
│   ├── starters.py    # High-level starters (e2e scenarios)
│── tests/             # Unit tests
│── examples/          # Usage examples
│── setup.py           # Python package setup
│── README.md          # Documentation
```

### **Node SDK (`node-sdk/`)**
```
node-sdk/
│── src/
│   ├── control.ts     # Control port client implementation
│   ├── process.ts     # Handles Anon binary execution
│   ├── socks.ts       # SOCKS5 client implementation
│   ├── proxy.ts       # Proxy management
│   ├── templates.ts   # Multi-step request templates
│   ├── starters.ts    # High-level starters (e2e scenarios)
│── tests/             # Unit tests
│── examples/          # Usage examples
│── package.json       # NPM package setup
│── README.md          # Documentation
```

---

## **Migration steps**
### **1. Python SDK migration**
- **Move class definitions to follow Stem structure** (`control.py`, `process.py`, etc.).
- **Replace direct Stem calls with wrapped classes**.
- **Migrate `socks_client.py` to `socks.py` for SOCKS handling**.
- **Migrate `anon_runner.py` to `process.py` for binary execution**.

### **2. Node SDK migration**
- **Follow the same structure as Python SDK.**
- **Migrate from `anon-{protocol}-client.ts` structure to `{protocol}.ts` for ControlPort, SOCKS, and binary management.**
- **Ensure an object-oriented API for better usability.**
