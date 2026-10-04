from .fastdata import iter_batches

def prefetch(it, depth=4):
    import queue
    import threading

    q = queue.Queue(maxsize=depth)
    done = object()

    def worker():
        try:
            for item in it:
                q.put(item)
            q.put(done)
        except BaseException as e:
            q.put(e)

    threading.Thread(
        target=worker,
        daemon=True,
    ).start()

    while True:
        item = q.get()

        if item is done:
            return

        if isinstance(item, BaseException):
            raise item

        yield item

