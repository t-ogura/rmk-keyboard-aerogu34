/* Seeed Xiao BLE (nRF52840), whose Adafruit bootloader ships with the s140
 * SoftDevice occupying 0x0..0x26000. The application therefore starts at
 * 0x27000 -- the same place ZMK loads on this board.
 *
 * The SoftDevice is left in place and unused: RMK drives the radio through
 * nrf-sdc and never enables it. Both Aerogu34 halves are Xiao BLEs, so this
 * one file serves both images.
 *
 * Length stops short of the storage partition at 0xEC000 ([storage] in
 * keyboard.toml puts RMK's sectors at 0xE4000..0xEC000, inside this range;
 * package.sh checks that no image reaches them).
 */
MEMORY
{
  FLASH : ORIGIN = 0x00027000, LENGTH = 788K
  RAM   : ORIGIN = 0x20000008, LENGTH = 255K
}
