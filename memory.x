/* Default memory layout for the SWM341 (512K flash @ 0x0, 64K SRAM @ 0x2000_0000).
 *
 * All figures come from the datasheet "features" chapter. When linking the final
 * application, provide your own `memory.x` in the search path to override these
 * `PROVIDE` values (e.g. parts with a different flash/SRAM split).
 */
PROVIDE(FLASH_BASE = 0x00000000);
PROVIDE(FLASH_SIZE = 512K);
PROVIDE(RAM_BASE = 0x20000000);
PROVIDE(RAM_SIZE = 64K);

MEMORY
{
  FLASH : ORIGIN = FLASH_BASE,   LENGTH = FLASH_SIZE
  RAM   : ORIGIN = RAM_BASE,     LENGTH = RAM_SIZE
}
