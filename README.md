# Room Reservation

A room booking API written in Rust. Users ask for one or more rooms for an event. The system checks the booking rules, finds conflicts and suggests free slots. Standard rooms are confirmed at once. Amphitheatres must be approved by a responsible user. A background task sends reminders, expires unanswered requests and frees rooms when nobody shows up.

The business process is modelled in BPMN ([`reservation-salles-bpmn.drawio`](reservation-salles-bpmn.drawio)). The code follows that diagram step by step.

## Features

- **Submit a booking**: one or more rooms, a date, a time slot and a number of participants.
- **Rule checks** (R1 to R6, listed below), with clear error codes.
- **Conflict detection**: if the slot is taken, the API suggests other free slots on the same day.
- **Approval flow**: standard rooms are confirmed automatically. Amphitheatres wait for a responsible user to approve or decline them.
- **Cancellation** by the person who booked, up to 2 hours before the start.
- **Check-in** at the room, from the start of the event up to 15 minutes after.
- **Automatic checks**, run every minute:
  - a reminder 1 hour before the start
  - a request expires after 24 hours without an answer
  - no check-in within 15 minutes counts as a no-show and frees the room
  - rooms are freed when the event ends
- **Room availability**: the taken slots of a room for a given day, so a client can draw a calendar.
- **Interactive API docs** with Swagger UI.

## Business rules

| Rule | Meaning | Example that fails |
|------|---------|--------------------|
| **R1**: valid slot | Between 07:00 and 19:00, lasting 30 min to 4 h, in 30-minute steps | 08:00–08:45; 06:30–08:00; 08:00–13:00 |
| **R2**: capacity | Participants ≤ total capacity of the booked rooms | 40 people in a 30-seat room |
| **R3**: no conflict | The slot must not overlap a pending or approved booking | 09:30–10:30 when 08:00–10:00 is taken |
| **R4**: lead time | Book at least 24 h and at most 30 days ahead | Monday 15:00 for Tuesday 10:00 (19 h ahead) |
| **R5**: approval | Standard room → confirmed at once; amphitheatre → a responsible user decides | Never fails: it only picks the path |
| **R6**: cancellation | Cancel up to 2 h before the start | Cancel at 08:30 a booking that starts at 10:00 |

The full table, with each rule's place in the BPMN diagram, is in [`regle_metiers.txt`](regle_metiers.txt).

## Architecture

The project follows a clean / hexagonal architecture ("ports and adapters"). Dependencies point inward only. The domain knows nothing about HTTP, storage or async.

```
┌────────────────────────────────────────────────────────────┐
│ infrastructure   axum HTTP API · in-memory repositories ·  │
│                  console notifier · system clock ·         │
│                  scheduler · Swagger UI                    │
│   ┌────────────────────────────────────────────────────┐   │
│   │ application  use cases (submit, validate, cancel…) │   │
│   │              ports: traits the infrastructure      │   │
│   │              implements (repositories, notifier,   │   │
│   │              clock)                                │   │
│   │   ┌────────────────────────────────────────────┐   │   │
│   │   │ domain   entities and rules R1–R6           │   │   │
│   │   │          (Booking, Room, User, System)      │   │   │
│   │   └────────────────────────────────────────────┘   │   │
│   └────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────┘
```

- **Domain** (`src/domain`): pure Rust. Holds the booking rules and state changes. No serde, no async.
- **Application** (`src/application`):
  - **Use cases**: one file per action. Each loads data through the ports, lets the domain decide, then saves and sends notifications.
  - **Ports**: async traits such as `BookingRepository` and `Notifier`.
- **Infrastructure** (`src/infrastructure`): the concrete adapters.
  - **Now**: in-memory storage, console notifications and the axum HTTP API.
  - **Later**: a database or an email sender can replace these without touching the domain or the use cases.

## Project layout

```
.
├── README.md
├── regle_metiers.txt              business rules R1–R6
├── reservation-salles-bpmn.drawio BPMN process diagram
└── room_booking/                  Rust crate
    ├── Cargo.toml
    ├── postman/                   Postman collection (end-to-end scenario)
    └── src/
        ├── main.rs                wiring + server start
        ├── domain/                entities and business rules
        ├── application/
        │   ├── ports/             traits implemented by the infrastructure
        │   └── use_cases/         one file per use case
        └── infrastructure/
            ├── http/              router, handlers, DTOs, errors, CORS, OpenAPI
            ├── in_memory/         in-memory repositories
            ├── scheduler.rs       periodic checks
            └── state.rs           shared state + adapter selection
```

## Getting started

**Requirements:** Rust 1.88 or newer (edition 2024).

```bash
cd room_booking
cargo run
```

The API listens on `http://127.0.0.1:3000`. Open **http://127.0.0.1:3000/docs** for Swagger UI.

### Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `ADDR` | `127.0.0.1:3000` | Address the server listens on |
| `CORS_ORIGINS` | `http://localhost:5173,http://127.0.0.1:5173` | Browser origins allowed to call the API (comma-separated) |

### Starting data

The server keeps its data in memory. Everything is reset on restart, and it starts with:

| Users | Rooms |
|-------|-------|
| `A-001` Alice (applicant) | `R-101` Salle 101: standard, 30 seats |
| `A-002` Bob (applicant) | `R-102` Salle 102: standard, 20 seats |
| `RESP-001` (responsible) | `R-001` Amphi A: amphitheatre, 100 seats |

## Using the API

There is no login yet. The caller gives their user id in the **`X-User-Id`** header. In Swagger UI, click **Authorize** and enter an id such as `A-001`.

```bash
curl -X POST http://127.0.0.1:3000/bookings \
  -H 'X-User-Id: A-001' -H 'Content-Type: application/json' \
  -d '{"room_ids":["R-101"],"participants":20,"event_date":"2026-12-01","event_start":"10:00","event_end":"11:00"}'
```

Replace `event_date` with a day between 1 and 30 days from now. Otherwise rule R4 rejects the request.

| Method | Path | Who |
|--------|------|-----|
| GET | `/rooms` | anyone |
| GET | `/rooms/{id}/availability?date=YYYY-MM-DD` | anyone |
| GET | `/users` | anyone (temporary, until real login) |
| POST | `/bookings` | applicant |
| GET | `/bookings/mine` | any user |
| GET | `/bookings?status=requested` | responsible |
| GET | `/bookings/{id}` | the booking's applicant, or a responsible |
| POST | `/bookings/{id}/validate` | responsible |
| POST | `/bookings/{id}/decline` | responsible |
| POST | `/bookings/{id}/cancel` | the booking's applicant |
| POST | `/bookings/{id}/check-in` | the booking's applicant |

**Errors** all use the same body:

```json
{ "error": "slot_unavailable", "message": "...", "alternatives": [{ "start": "11:00:00", "end": "12:00:00" }] }
```

`error` is a stable code that clients should test. `message` is readable text and may change. `alternatives` only appears for `slot_unavailable`.

**Status codes:**

| Code | Meaning | Error codes |
|------|---------|-------------|
| 401 | missing or unknown user | |
| 403 | not allowed | |
| 404 | not found | |
| 409 | the current state forbids it | `slot_unavailable`, `cancellation_too_late` |
| 422 | invalid request | `capacity_exceeded` |
| 500 | storage failure | |

Swagger UI lists every endpoint with its full set of error codes. The raw OpenAPI spec is at `/api-docs/openapi.json`.

## Tests

```bash
cd room_booking
cargo test
```

The tests cover:

- **Business rules**: every rule and state change in the domain.
- **Use cases**: each one runs against in-memory repositories, a fixed clock and a fake notifier.
- **HTTP API**: every endpoint, the error format, CORS, and a check that the API docs match the real routes.

**Postman:** an end-to-end scenario runs against a live server. It can be re-run because it cleans up after itself.

```bash
cargo run    # in another terminal
postman collection run postman/room_booking.postman_collection.json
```

## Roadmap

- [ ] Frontend (React + TypeScript, client generated from the OpenAPI spec)
- [ ] Database (SQLite or PostgreSQL) instead of in-memory storage
- [ ] Real login, replacing the `X-User-Id` header
- [ ] Email notifications instead of console output
- [ ] Pagination for the booking lists

## License

[MIT](LICENSE) © 2026 Nomeny Mitia ANDRIAMAHEVA
