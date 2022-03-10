MEMORY
{
  /* "Internal Flash" */
  /* TODO: This should be 2MiB? */
  FLASH : ORIGIN = 0x00400000, LENGTH = 1M

  /* "Internal Flash" */
  /* TODO: This should be 384KiB? */
  RAM : ORIGIN = 0x20401000, LENGTH = 256K
}
