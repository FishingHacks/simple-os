# Simple File System

i know, im a genius in names (im really not)

# Blocks

Alright, so, a disk is split into multiple 4-KiB (4096 bytes) **blocks**. At the start of a **block array** (16384 blocks, 2048 \* 8), a block is specified to be the **block array descriptor**, it holds the status to each block (is used or not). The structure has 4096 8-bit bitmaps 2048 of those determining if the block is allocated, the rest the type of the block.

The memory layout looks like this:

```
┌────────────────────────┬──────────┬──────────┬──────────┬──>
│ Block Array Descriptor │ Block #1 │ Block #2 │ Block #2 │ ...
└────────────────────────┴──────────┴──────────┴──────────┴──>
```

To get the status of a particular block, you read the block array descriptor as its origin, which can be calculated using the following expression: `let block_array_descriptor = block_id / 16384 * 16384`. Note: This is supposed to be a flooring-integer division. To then get the index in that block and its bitmap, you do:

```rs
let local_block_id = block_id % 16384;
let block_index = local_block_id / 8;
let bitmap_offset = local_block_id % 8;
```

You can then use these values to check if the block is in use:

```rs
#[repr(C)]
struct BitmapField([u8; 2048])

let block_array_descriptor = block_id / 16384 * 16384;
let local_block_id = block_id % 16384;
let block_index = local_block_id / 8;
let bitmap_offset = local_block_id % 8;

let usage_bitmap = read_block::<BitmapField>(block_array_descriptor).0[block_offset];
let type_bitmap = read_block::<BitmapField>(block_array_descriptor + 2048).0[block_offset];

// note: this could be expressed in an enum value: Unused, Allocated, InodeBlock, BlockArrayDescriptor
// block_id % 16384 == 0 => BlockArrayDescriptor
// is_used: false => Unused,
// is_used: true, is_inode_block => InodeBlock
// is_used: true, is_inode_block: false => Allocated
let is_inode_block = (type_bitmap & (1 << bitmap_offset)) > 0;
let is_used = (bitmap & (1 << bitmap_offset)) > 0 || block_id % 16384 == 0;
```

Notice: we have to check `block_id % 32768 == 0`, because if the block is at index 0 into the block array, then that means its the block array descriptor, which is **always** in use.

# Superblock

At block index 1 you have the Superblock. It holds the metadata for the file system and is 4 KiB in size:

| Name                 | Offset (bytes) | Size (bytes) |                                                                  Description |
| :------------------- | :------------- | :----------- | ---------------------------------------------------------------------------: |
| Signature            | 0              | 8            |             The 8-byte sfs signature: 0x5346732073626x6b (string "SFs sblk") |
| Earliest Unused      | 8              | 4            |                                 The block address for the first unused block |
| Earliest Inode Space | 8              | 4            | The block address for the first inode block that has space to fit more nodes |
| Latest Unused        | 12             | 4            |                                  The block address for the last unused block |
| Total Unused         | 18             | 4            |                                            The total number of unused blocks |
| Total Blocks         | 22             | 4            |                                              The total number of used blocks |
| Last Mount           | 26             | 8            |                                                  The last mount in UNIX-Time |
| Last Write           | 34             | 8            |                                                  The last write in UNIX-Time |
| Name                 | 42             | 32           |  The 32 long name, ends at either the 32th character or first zero character |
| PreallocFiles        | 74             | 1            |                    The number of blocks to preallocate for files (usually 1) |
| PreallocDirs         | 75             | 1            |              The number of blocks to preallocate for directories (usually 1) |
| Padding              | 76             | X .. 4096    |                The padding to make the superblock 4 KiB long, should be zero |

The first step of initializing the file system is reading this block. It should be stored for future references.

# Accessing Files

SFS has a concept called Inodes: They're like metadata, they hold data for the file (most notably tho, not the name, why that is is explained on later).

An Inode has a number of blocks associated to it, which hold its contents.

An Inode structure is 128 bytes and looks the following:

| Name                          | Offset (bytes) | Size (bytes) |                                                                                                              Description |
| :---------------------------- | :------------- | :----------- | -----------------------------------------------------------------------------------------------------------------------: |
| Type and Permission           | 0              | 2            |                                                               The type and permission bitfield of this inode (see below) |
| User ID                       | 2              | 2            |                                                                                     The ID of the user owning this inode |
| Group ID                      | 4              | 2            |                                                                                    The ID of the group owning this inode |
| Access Time                   | 6              | 8            |                                                                           The last access time of this inode (UNIX-Time) |
| Modification Time             | 14             | 8            |                                                                     The last modification time of this inode (UNIX-Time) |
| Creation Time                 | 22             | 8            |                                                                              The time this inode was created (UNIX-Time) |
| Hardlinks                     | 30             | 2            | The number of hard links (directory entries) linking to this inode. Once this number reaches 0, the inode is unallocated |
| Direct Block Pointer 0        | 32             | 4            |                                                                                            The first block of this inode |
| Direct Block Pointer 1        | 36             | 4            |                                                                                           The second block of this inode |
| Direct Block Pointer 2        | 40             | 4            |                                                                                            The third block of this inode |
| Direct Block Pointer 3        | 44             | 4            |                                                                                           The fourth block of this inode |
| Direct Block Pointer 4        | 48             | 4            |                                                                                            The fifth block of this inode |
| Direct Block Pointer 5        | 52             | 4            |                                                                                            The sixth block of this inode |
| Direct Block Pointer 6        | 56             | 4            |                                                                                          The seventh block of this inode |
| Direct Block Pointer 7        | 60             | 4            |                                                                                           The eighth block of this inode |
| Direct Block Pointer 8        | 64             | 4            |                                                                                            The ninth block of this inode |
| Direct Block Pointer 9        | 68             | 4            |                                                                                            The tenth block of this inode |
| Singly Indirect Block Pointer | 72             | 4            |                                                        A block containing a list of block pointers (1024 block pointers) |
| Dobly Indirect Block Pointer  | 76             | 4            |                                                        A block containing a list of block pointers (1024 block pointers) |
| Meta                          | 80             | 4            |                                                                                         A 32-bit meta number (see below) |
| Padding                       | 84             | X..128       |                                                                        The padding to make the superblock 128 bytes long |

A Block can contain up to 32 inodes.

### Block and Type bitfields

The type bitfield occupies the upper 4 bits:

| Type value in Hex | Type Description |
| ----------------- | ---------------- |
| 0x1000            | FIFO             |
| 0x2000            | Character Device |
| 0x4000            | Directory        |
| 0x6000            | Block Device     |
| 0x8000            | File             |
| 0xa000            | Socket           |

### Meta Number

| File Type        | Metanumber Meaning                     |
| ---------------- | -------------------------------------- |
| FIFO             | _unused_                               |
| Character Device | Device ID                              |
| Block Device     | Device ID                              |
| Directory        | Number of entries in the last block    |
| File             | number of bytes used in the last block |
| Socket           | Socket ID                              |

Permissions occupy the lower 12 bits:

| Permission in octal | Permission Description |
| ------------------- | ---------------------- |
| 0001                | Other - execute        |
| 0002                | Other - Write          |
| 0004                | Other - Read           |
| 0010                | Group - execute        |
| 0020                | Group - Write          |
| 0040                | Group - Read           |
| 0100                | User - execute         |
| 0200                | User - Write           |
| 0400                | User - Read            |
| 1000                | Sticky Bit             |
| 2000                | Set group ID           |
| 4000                | Set user ID            |

## Reading the contents of an inode

If you have the inode, reading it is not very hard. Note: You cannot have a file of size >4235264 bytes (4.23 MiB) (1024 + 10 blocks) because there are only 1034 possible blocks per inode (10 in the inode itself, direct block pointer 0 - 9, 1024 in the singly indirect block pointer)

If the block id <= 9, just read the block in the inode at that direct block pointer

If the block id > 9 and < 1034, you have to read the singly indirect block pointer and then read the block pointer at offset #block_id-10.

If the block is >= 1034 and < 1048586 (1024 \* 1024 + 10), you have to read the singly indirect pointer at offset #(block_id-10)/1024 and then read the block pointer at offset #(block_id-10)%1024.

Continue like that for the nth indirect block pointer: (block_id >= 10 + 1024 ^ (n - 1) and < 10 + 2014 ^ n), read the n-1th block at #(block_id-10)/(1024 ^ (n-1)) and then read from there #(block_id-10)%(1024 ^ (n-1)). Rinse and repeat.

## Reading a directory inode

A directory inode also has a list of allocated blocks, but in this case, the allocated blocks just contain a number of DirEntry Structures. Note: A direntry structure can **never** be at the address 4096-structlen or later.

DirEntry Struct:

| Name | Offset (bytes) | Size (bytes) |                                     Description |
| :--- | :------------- | :----------- | ----------------------------------------------: |
| Size | 0              | 1            |                 The length of the name (1..127) |
| Id   | 1              | 4            | The ID of the Inode that this direntry links to |
| Name | 5              | N            |                          The name of this entry |
