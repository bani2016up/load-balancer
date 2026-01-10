from fastapi import FastAPI
import asyncio

app = FastAPI()

@app.get("/")
async def root():
    return {"message": f"Replica on port 3000"}


async def app(scope, receive, send):
    await send({
        'type': 'http.response.start',
        'status': 200,
        'headers': [(b'content-type', b'text/plain')],
    })
    await send({
        'type': 'http.response.body',
        'body': b'Fast async response',
    })
