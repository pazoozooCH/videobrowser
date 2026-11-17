import { decodeName, decodePath } from './encoded-name';

describe('decodeName', () => {
  it('decodes an encoded name', () => {
    expect(decodeName('.dat_VGVzdA==')).toBe('Test');
  });

  it('decodes UTF-8, as the backend encodes it', () => {
    expect(decodeName('.dat_w6nDoMOoISvDpyUmL17CqHcg')).toBe('éàè!+ç%&/^¨w ');
  });

  it('leaves plain and undecodable names as they are', () => {
    expect(decodeName('movie.mp4')).toBe('movie.mp4');
    expect(decodeName('.dat_!!!')).toBe('.dat_!!!');
    expect(decodeName('.dat_/w==')).toBe('.dat_/w==');
  });
});

describe('decodePath', () => {
  it('decodes every encoded segment', () => {
    expect(decodePath('.dat_VGVzdA==/plain/.dat_w6nDoMOoISvDpyUmL17CqHcg')).toBe(
      'Test/plain/éàè!+ç%&/^¨w ',
    );
  });

  it('accepts backslashes', () => {
    expect(decodePath('a\\.dat_VGVzdA==')).toBe('a/Test');
  });
});
