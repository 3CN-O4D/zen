# Chapter 48: Capstone Projects

Put everything together with these complete, real-world projects.

## Project 1: Full-Stack Blog API

Build a complete blog API with authentication, posts, comments.

```zen
import http
import json
import crypto.jwt
import crypto.rand

let JWT_SECRET = "change-in-production"
let posts = []
let users = []

func hash_password(pw) {
    # Simplified
    return crypto.hash.sha256(pw).hexdigest()
}

func auth_middleware(req, handler) {
    if req.path.starts_with("/api/protected") {
        let auth = req.headers.get("Authorization", "")
        if !auth.starts_with("Bearer ") {
            return {"status": 401, "body": json.stringify({"error": "Unauthorized"})}
        }
        try {
            let token = auth.split(" ")[1]
            let payload = crypto.jwt.decode(token, JWT_SECRET)
            req.user = payload
        } catch {
            return {"status": 401, "body": json.stringify({"error": "Invalid token"})}
        }
    }
    return handler(req)
}

func router(req) {
    # Register
    if req.path == "/api/register" && req.method == "POST" {
        let body = json.parse(req.body)
        let user = {
            "id": len(users) + 1,
            "username": body.username,
            "password": hash_password(body.password)
        }
        users.push(user)
        return {"status": 201, "body": json.stringify({"id": user.id})}
    }
    
    # Login
    if req.path == "/api/login" && req.method == "POST" {
        let body = json.parse(req.body)
        for user in users {
            if user.username == body.username && user.password == hash_password(body.password) {
                let token = crypto.jwt.encode({"user_id": user.id}, JWT_SECRET)
                return {"status": 200, "body": json.stringify({"token": token})}
            }
        }
        return {"status": 401, "body": json.stringify({"error": "Invalid credentials"})}
    }
    
    # Get posts
    if req.path == "/api/posts" && req.method == "GET" {
        return {"status": 200, "body": json.stringify(posts)}
    }
    
    # Create post (protected)
    if req.path == "/api/protected/posts" && req.method == "POST" {
        let body = json.parse(req.body)
        let post = {
            "id": len(posts) + 1,
            "title": body.title,
            "content": body.content,
            "author_id": req.user.user_id
        }
        posts.push(post)
        return {"status": 201, "body": json.stringify(post)}
    }
    
    return {"status": 404, "body": json.stringify({"error": "Not found"})}
}

func main() {
    print("Blog API running on http://localhost:8080")
    http.serve("0.0.0.0", 8080, fn(req) { return auth_middleware(req, router) })
}

main()
```

## Project 2: Real-Time Chat Application

```zen
import socket
import json

let clients = []
let client_counter = 0

func broadcast(message, sender_id) {
    for client in clients {
        if client.id != sender_id {
            try {
                socket.send(client.sock, json.stringify(message) + "\n")
            } catch {}
        }
    }
}

func handle_client(client) {
    while true {
        try {
            let data = socket.recv(client.sock, 1024)
            if len(data) == 0 {
                break
            }
            let msg = json.parse(data.trim())
            broadcast({
                "type": "message",
                "from": client.name,
                "text": msg.text
            }, client.id)
        } catch {
            break
        }
    }
    # Remove client
    socket.close(client.sock)
    print(client.name + " disconnected")
}

func main() {
    let server = socket.create(socket.AF_INET, socket.SOCK_STREAM)
    socket.setsockopt(server, socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    socket.bind(server, "0.0.0.0", 9000)
    socket.listen(server, 100)
    
    print("Chat server on port 9000")
    
    while true {
        let sock = socket.accept(server)
        client_counter = client_counter + 1
        let client = {"id": client_counter, "sock": sock, "name": "User" + client_counter}
        clients.push(client)
        print("New connection: " + client.name)
        spawn(fn() { handle_client(client) })
    }
}

main()
```

## Project 3: Web Scraper with Data Export

```zen
import requests
import json
import re
import time

class Scraper {
    func init() {
        self.results = []
    }
    
    func scrape_quotes() {
        let page = 1
        while page <= 3 {
            let url = "https://quotes.toscrape.com/page/" + page + "/"
            try {
                let resp = requests.get(url)
                # Extract quotes (simplified pattern)
                let quotes = re.findall(r'<span class="text" itemprop="text">([^<]+)</span>', resp.text)
                let authors = re.findall(r'<small class="author" itemprop="author">([^<]+)</small>', resp.text)
                
                let i = 0
                while i < len(quotes) {
                    self.results.push({
                        "quote": quotes[i],
                        "author": authors[i]
                    })
                    i = i + 1
                }
                print("Scraped page " + page)
                time.sleep(1)
            } catch (e) {
                print("Error: " + e)
            }
            page = page + 1
        }
    }
    
    func export_json(filename) {
        with open(filename, "w") as f {
            f.write(json.stringify(self.results, indent=2))
        }
        print("Exported to " + filename)
    }
    
    func export_csv(filename) {
        let content = "quote,author\n"
        for r in self.results {
            content = content + '"' + r.quote.replace('"', '""') + '","' + r.author.replace('"', '""') + '"\n'
        }
        fs.write(filename, content)
        print("Exported to " + filename)
    }
}

let scraper = Scraper()
scraper.scrape_quotes()
scraper.export_json("quotes.json")
scraper.export_csv("quotes.csv")
```

## Project 4: Task Scheduler

```zen
import time
import json

class TaskScheduler {
    func init() {
        self.tasks = []
    }
    
    func add_task(name, func, interval_seconds) {
        self.tasks.push({
            "name": name,
            "func": func,
            "interval": interval_seconds,
            "last_run": 0
        })
        print("Added task: " + name)
    }
    
    func run() {
        print("Task scheduler started")
        while true {
            let now = time.time()
            for task in self.tasks {
                if now - task.last_run >= task.interval {
                    print("Running task: " + task.name)
                    try {
                        task.func()
                    } catch (e) {
                        print("Task error: " + e)
                    }
                    task.last_run = now
                }
            }
            time.sleep(1)
        }
    }
}

let scheduler = TaskScheduler()

scheduler.add_task("heartbeat", fn() {
    print("Heartbeat: " + time.time())
}, 10)

scheduler.add_task("cleanup", fn() {
    print("Cleaning up...")
}, 60)

scheduler.run()
```

## Final Words

Congratulations on completing the comprehensive Zen for Dummies guide! You've learned:

- **Fundamentals**: Data types, functions, lambdas, closures
- **Core Concepts**: Modules, error handling, classes, objects
- **Networking**: Sockets, HTTP, HTTPS, SSL/TLS, DNS
- **Concurrency**: Threads, channels, mutexes
- **Cryptography**: Hashing, encryption, JWT, certificates
- **Practical Skills**: Web scraping, API clients, file processing
- **Projects**: Real-world applications

## Next Steps

1. **Practice** - Build your own projects
2. **Read the Source** - Explore Zen's stdlib and codebase
3 **Document** - Write comments and documentation
4. **Share** - Contribute to the Zen community
5. **Keep Learning** - Experiment with new features

Remember: The best way to learn is by doing. Start with small projects and gradually build up to more complex ones.

**Happy coding with Zen!**

*End of Zen for Dummies: Complete Edition*
