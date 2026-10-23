# Chapter 45: REST API Client

Building a robust REST API client is a practical skill. This chapter walks through creating a reusable API client in Zen.

## Understanding REST APIs

REST (Representational State Transfer) uses HTTP methods to perform CRUD operations:
- **Create**: POST
- **Read**: GET
- **Update**: PUT/PATCH
- **Delete**: DELETE

## Basic API Client Structure

```zen
import requests
import json

class ApiClient {
    func init(base_url, api_key = null) {
        self.base_url = base_url
        self.api_key = api_key
        self.session = requests.Session()
        if api_key != null {
            self.session.headers["Authorization"] = "Bearer " + api_key
        }
        self.session.headers["Content-Type"] = "application/json"
        self.session.headers["User-Agent"] = "Zen-API-Client/1.0"
    }
    
    func get(endpoint, params = {}) {
        let url = self.base_url + endpoint
        try {
            let resp = self.session.get(url, params=params)
            return self._handle_response(resp)
        } catch (e) {
            throw "GET request failed: " + e
        }
    }
    
    func post(endpoint, data = {}) {
        let url = self.base_url + endpoint
        try {
            let resp = self.session.post(url, json=data)
            return self._handle_response(resp)
        } catch (e) {
            throw "POST request failed: " + e
        }
    }
    
    func put(endpoint, data = {}) {
        let url = self.base_url + endpoint
        try {
            let resp = self.session.put(url, json=data)
            return self._handle_response(resp)
        } catch (e) {
            throw "PUT request failed: " + e
        }
    }
    
    func delete(endpoint) {
        let url = self.base_url + endpoint
        try {
            let resp = self.session.delete(url)
            return self._handle_response(resp)
        } catch (e) {
            throw "DELETE request failed: " + e
        }
    }
    
    func _handle_response(resp) {
        if resp.status_code >= 200 && resp.status_code < 300 {
            if resp.text == "" {
                return null
            }
            try {
                return json.parse(resp.text)
            } catch {
                return resp.text
            }
        } elif resp.status_code == 401 {
            throw "Unauthorized: " + resp.text
        } elif resp.status_code == 404 {
            throw "Not Found: " + resp.text
        } elif resp.status_code >= 400 && resp.status_code < 500 {
            throw "Client Error " + resp.status_code + ": " + resp.text
        } elif resp.status_code >= 500 {
            throw "Server Error " + resp.status_code + ": " + resp.text
        }
        throw "Unexpected status: " + resp.status_code
    }
}
```

## Using the API Client

```zen
let client = ApiClient("https://jsonplaceholder.typicode.com")

# GET all posts
let posts = client.get("/posts")
print("Got " + len(posts) + " posts")

# GET single post
let post = client.get("/posts/1")
print(post.title)

# POST create post
let new_post = client.post("/posts", {
    "title": "Hello Zen",
    "body": "This is a post from Zen",
    "userId": 1
})
print("Created post with id: " + new_post.id)

# PUT update post
let updated = client.put("/posts/1", {
    "id": 1,
    "title": "Updated Title",
    "body": "Updated body",
    "userId": 1
})
print(updated.title)

# DELETE post
client.delete("/posts/1")
print("Deleted")
```

## Adding Authentication

```zen
# API key auth
let client = ApiClient("https://api.example.com", "your-api-key-here")

# Or OAuth token
let client2 = ApiClient("https://api.example.com")
client2.session.headers["Authorization"] = "Bearer " + oauth_token
```

## Error Handling and Retries

```zen
import time

class RobustApiClient extends ApiClient {
    func init(base_url, api_key = null, max_retries = 3) {
        super.init(base_url, api_key)
        self.max_retries = max_retries
    }
    
    func get(endpoint, params = {}) {
        return self._request_with_retry(fn() {
            return super.get(endpoint, params)
        })
    }
    
    func _request_with_retry(func) {
        let attempts = 0
        while attempts < self.max_retries {
            try {
                return func()
            } catch (e) {
                attempts = attempts + 1
                if attempts >= self.max_retries {
                    throw e
                }
                print("Retry " + attempts + " after error: " + e)
                time.sleep(2 * attempts)  # Exponential backoff
            }
        }
    }
}
```

## Pagination Handling

```zen
class PaginatedClient extends ApiClient {
    func get_all(endpoint, params = {}) {
        let all_results = []
        let page = 1
        let per_page = 100
        
        while true {
            let page_params = params.copy()
            page_params["page"] = page
            page_params["per_page"] = per_page
            
            let results = self.get(endpoint, page_params)
            if len(results) == 0 {
                break
            }
            
            for item in results {
                all_results.push(item)
            }
            
            if len(results) < per_page {
                break
            }
            
            page = page + 1
        }
        
        return all_results
    }
}
```

## Rate Limiting

```zen
import time

class RateLimitedClient extends ApiClient {
    func init(base_url, api_key = null, requests_per_second = 10) {
        super.init(base_url, api_key)
        self.delay = 1.0 / requests_per_second
        self.last_request = 0
    }
    
    func _throttle() {
        let now = time.time()
        if now - self.last_request < self.delay {
            time.sleep(self.delay - (now - self.last_request))
        }
        self.last_request = time.time()
    }
    
    func get(endpoint, params = {}) {
        self._throttle()
        return super.get(endpoint, params)
    }
    
    func post(endpoint, data = {}) {
        self._throttle()
        return super.post(endpoint, data)
    }
}
```

## Building a GitHub API Client

```zen
class GitHubClient extends ApiClient {
    func init(token = null) {
        super.init("https://api.github.com", token)
    }
    
    func get_user(username) {
        return self.get("/users/" + username)
    }
    
    func get_repos(username) {
        return self.get("/users/" + username + "/repos", {"sort": "stars"})
    }
    
    func get_repo(owner, repo) {
        return self.get("/repos/" + owner + "/" + repo)
    }
    
    func create_issue(owner, repo, title, body = "") {
        return self.post("/repos/" + owner + "/" + repo + "/issues", {
            "title": title,
            "body": body
        })
    }
}

# Usage
let gh = GitHubClient("your-github-token")
let user = gh.get_user("octocat")
print(user.name)
let repos = gh.get_repos("octocat")
for repo in repos[0:5] {
    print(repo.name + " - " + repo.stargazers_count)
}
```

## Testing API Clients

```zen
# Mock responses for testing
class MockApiClient {
    func get(endpoint, params = {}) {
        if endpoint == "/users/1" {
            return {"id": 1, "name": "Test User"}
        }
        throw "Not found"
    }
}

# Use in tests
let mock = MockApiClient()
let user = mock.get("/users/1")
print(user.name)
```

## Best Practices

1. **Use a base class/client** - DRY principle
2. **Handle authentication** - Store tokens securely
3. **Error handling** - Catch and provide meaningful errors
4. **Rate limiting** - Respect API limits
5. **Retries** - For transient failures
6. **Pagination** - Handle large result sets
7. **Timeouts** - Set reasonable timeouts
8. **Logging** - Log requests/responses for debugging
9. **Type safety** - Validate responses
10. **Environment variables** - Don't hardcode keys

## Chapter Summary

- Build reusable API client classes
- Handle all HTTP methods
- Implement authentication, retries, pagination
- Add rate limiting and error handling
- Use sessions for efficiency
- Test with mocks
- Follow API best practices

**Next:** Chapter 46 - File Processing Tools
