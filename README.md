# nvme-async

## Example
The following minimal example shows how this library can be used to asynchronous block I/O
using the `io_uring` NVMe passthrough command.
```rust


#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use io_uring::{squeue::Entry128, cqueue::Entry32};
    use io_uring_async::IoUringAsync;
    use send_wrapper::SendWrapper;
    use crate::nvme::{NvmeBlockDevice};
    use bdev_async::bdev::{BlockDeviceQueue, BlockDevice};

    #[test]
    fn test_1() {
        let uring: IoUringAsync<Entry128, Entry32> = IoUringAsync::generic_new(8).unwrap();
        let uring = Rc::new(uring);

        // Create a new current_thread runtime that submits all outstanding submission queue
        // entries as soon as the executor goes idle.
        let uring_clone = SendWrapper::new(uring.clone());
        let runtime = tokio::runtime::Builder::new_current_thread().
            on_thread_park(move || { uring_clone.submit().unwrap(); }).
            enable_all().
            build().unwrap();  

        runtime.block_on(async move {
            tokio::task::LocalSet::new().run_until(async {
                tokio::task::spawn_local(IoUringAsync::listen(uring.clone()));

                let bdev = NvmeBlockDevice::open("/dev/ng0n1").unwrap();
                println!("block size: {} bytes", 1 << bdev.logical_block_size());
                println!("n_blocks: {}", bdev.size());

                let bdev_queue = bdev.create_queue(uring);
                let mut buf = vec![0u8; 512];
                let res = bdev_queue.write_at(&mut buf, 0).await.unwrap();
                println!("{} bytes written: {:?}", res, buf);

                let mut buf = vec![0u8; 512];
                let res = bdev_queue.read_at(&mut buf, 0).await.unwrap();
                println!("{} bytes read: {:?}", res, buf);
            }).await; 
        });
    }
}
```