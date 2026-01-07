# MarbleChain

Two player cooperative Zuma-type marble matching puzzle game. Game is simple: shoot your marble into a spot in the chain which has 2 or more marbles of the same color repeating.  This prototype was made for a university class.


## Features

- Lobby, room system
- Server authoritative two player cooperative gameplay
- Endless mode - collect a high score while the chain gets faster
- Two levels




## Tech Stack

**Client:** ThreeJS

**Server:** Rust, Tokio, Axum, 

**Transport:** Websockets

**Serialization:** JSON


## Deployment

To deploy this project run

Client (locally):
```bash
  npx vite
```

Server (locally):
```bash
  cargo run
```


## Screenshots
<img width="1916" height="914" alt="first" src="https://github.com/user-attachments/assets/27c8dfde-9448-41c1-8d83-d365001ed9fc" />

<img width="1916" height="914" alt="second" src="https://github.com/user-attachments/assets/4dbbf377-7aa0-47ff-a866-9b857d98efd4" />
