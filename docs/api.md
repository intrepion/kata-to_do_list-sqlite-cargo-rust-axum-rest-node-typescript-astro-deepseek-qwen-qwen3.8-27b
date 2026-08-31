# API Contract

Version 1 — the contract both sides implement: the Axum server (M2) and the TypeScript client (M3) code against this document, and the M4 smoke test runs against it.

- No auth, no pagination: a single-user flat list (ADR 0003).
- All request/response bodies are JSON with `snake_case` fields.
- Wrong verb for a path → `405` with an `Allow` header. Non-`application/json` body on POST/PATCH → `415`.
- Unknown JSON fields in create/update bodies are **ignored**.
- Timestamps are RFC 3339 UTC with millisecond precision: `2026-08-30T10:00:00.000Z`.
- `updated_at` is bumped on every successful PATCH — even a no-op write — and is always ≥ `created_at`.

## OpenAPI

```yaml
openapi: 3.1.0
info:
  title: Todo List API
  version: 1.0.0
  description: >
    REST API for a single-user To Do List: one flat collection of todos.
    No authentication. All bodies are JSON.
servers:
  - url: http://localhost:3000
    description: The app's server, in both dev and production.
paths:
  /api/todos:
    get:
      summary: List todos
      operationId: listTodos
      parameters:
        - name: status
          in: query
          required: false
          description: Filter by completion state. Defaults to `all`.
          schema:
            type: string
            enum: [all, active, done]
            default: all
      responses:
        "200":
          description: Todos, oldest first (created_at, id).
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: "#/components/schemas/Todo"
        "400":
          $ref: "#/components/responses/BadRequest"
    post:
      summary: Create a todo
      operationId: createTodo
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: "#/components/schemas/TodoCreate"
      responses:
        "201":
          description: The created todo.
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Todo"
        "400":
          $ref: "#/components/responses/BadRequest"
  /api/todos/{id}:
    parameters:
      - name: id
        in: path
        required: true
        schema:
          type: integer
          format: int64
          minimum: 1
    get:
      summary: Get one todo
      operationId: getTodo
      responses:
        "200":
          description: The todo.
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Todo"
        "404":
          $ref: "#/components/responses/NotFound"
    patch:
      summary: Partially update a todo
      operationId: updateTodo
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: "#/components/schemas/TodoUpdate"
      responses:
        "200":
          description: The updated todo (updated_at bumped).
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Todo"
        "400":
          $ref: "#/components/responses/BadRequest"
        "404":
          $ref: "#/components/responses/NotFound"
    delete:
      summary: Delete a todo (hard delete; done todos are never auto-deleted)
      operationId: deleteTodo
      responses:
        "204":
          description: Deleted. No body.
        "404":
          $ref: "#/components/responses/NotFound"
components:
  schemas:
    Todo:
      type: object
      required: [id, title, description, done, created_at, updated_at]
      properties:
        id:
          type: integer
          format: int64
        title:
          type: string
          minLength: 1
          maxLength: 200
        description:
          type: [string, "null"]
          maxLength: 2000
        done:
          type: boolean
        created_at:
          type: string
          format: date-time
        updated_at:
          type: string
          format: date-time
    TodoCreate:
      description: A new todo. `description` absent is treated as `null`.
      type: object
      required: [title]
      properties:
        title:
          type: string
          minLength: 1
          maxLength: 200
        description:
          type: [string, "null"]
          maxLength: 2000
    TodoUpdate:
      description: >
        All fields optional; absent fields are left unchanged. For
        `description`: a string sets it, `null` clears it.
      type: object
      properties:
        title:
          type: string
          minLength: 1
          maxLength: 200
        description:
          type: [string, "null"]
          maxLength: 2000
        done:
          type: boolean
    Error:
      type: object
      required: [error]
      properties:
        error:
          type: string
  responses:
    BadRequest:
      description: Validation failed (bad body or bad query).
      content:
        application/json:
          schema:
            $ref: "#/components/schemas/Error"
          example:
            error: "title must be 1..=200 characters"
    NotFound:
      description: No such todo.
      content:
        application/json:
          schema:
            $ref: "#/components/schemas/Error"
          example:
            error: "todo 42 not found"
```

## The `description` field, precisely

The one non-trivial part of the contract, and why it is `PATCH` (partial update) rather than `PUT` (full replace):

- **POST** — field absent → `null`.
- **PATCH** — field absent → unchanged; `null` → cleared; string → set.
- **Responses** — `"description": null` when the todo has no description (the key is always present).

## Errors

| status | when | body |
|---|---|---|
| 400 | invalid title/description/status | `{"error": "<what failed>"}` |
| 404 | no todo with that id | `{"error": "todo <id> not found"}` |
| 405 | verb not allowed on the path | Axum default, with `Allow` header |
| 415 | non-JSON request body | Axum default |
| 500 | anything unexpected | `{"error": "internal error"}` — details go to server logs only |

## Examples

```sh
# create (201)
curl -s localhost:3000/api/todos -X POST \
  -H 'content-type: application/json' \
  -d '{"title":"Write docs","description":"M1"}'

# list (200)
curl -s localhost:3000/api/todos

# complete (200; reopen with {"done":false})
curl -s localhost:3000/api/todos/1 -X PATCH -d '{"done":true}'

# delete (204)
curl -sI localhost:3000/api/todos/1 -X DELETE
```
